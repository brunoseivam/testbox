use std::{error::Error, time::Duration, iter::zip};

use log::{info, debug};
use tokio::{sync::mpsc, select, time};
use lazy_static::lazy_static;
use rand::random;

use crate::parser::{Request, RequestNoun, Response, ResponseError};

struct Positioner {
    min: i64,
    max: i64,
    def: i64,
    value: i64
}

#[derive(Debug)]
pub(crate) struct PositionerState {
    pub value: i64,
}

impl Positioner {
    fn new(min: i64, max: i64, def: i64) -> Self {
        Positioner {
            min, max, def,
            value: def
        }
    }

    fn get(&self) -> PositionerState {
        PositionerState { value: self.value }
    }

    fn set(&mut self, new_value: i64) -> PositionerState {
        self.value = i64::max(i64::min(new_value, self.max), self.min);
        self.get()
    }

    fn set_max(&mut self) -> PositionerState {
        self.set(self.max)
    }

    fn set_min(&mut self) -> PositionerState {
        self.set(self.min)
    }

    fn reset(&mut self) -> PositionerState {
        self.value = self.def;
        self.get()
    }
}


struct Sensor {
    status: String,
    temperature: f64,
    humidity: f64,
    last_update: time::Instant,
}

#[derive(Debug)]
pub(crate) struct SensorState {
    pub status: String,
    pub temperature: f64,
    pub humidity: f64,
}

impl Sensor {
    fn new() -> Self {
        Self {
            status: "OK".into(),
            temperature: 20.0,
            humidity: 50.0,
            last_update: time::Instant::now(),
        }
    }

    fn get(&self) -> SensorState {
        SensorState {
            status: self.status.clone(),
            temperature: self.temperature,
            humidity: self.humidity,
        }
    }

    fn update(&mut self, now: &time::Instant) -> bool {
        let elapsed = now.duration_since(self.last_update);

        // Read temperature sensor every 2 seconds
        if elapsed >= Duration::from_millis(2000) {
            self.last_update = *now;

            self.temperature = random::<f64>()*10.0 + 20.0; // random temp between 20 and 30 deg
            self.humidity = random::<f64>()*40.0 + 30.0; // random humidity between 30 and 70
            debug!("New sensor reading: temp={:.2}, hum={:.2}", self.temperature, self.humidity);
            true
        } else {
            false
        }
    }
}

enum SelfTestCmd {
    Min,
    Max,
    Def,
}

struct SelfTestStep([SelfTestCmd; 4], time::Duration);

lazy_static! {
    static ref SELF_TEST: Vec<SelfTestStep> = vec![
        // Red LED, Yellow LED, Green LED, Servo
        SelfTestStep([SelfTestCmd::Def, SelfTestCmd::Def, SelfTestCmd::Def, SelfTestCmd::Def], Duration::from_millis(500)),
        SelfTestStep([SelfTestCmd::Max, SelfTestCmd::Min, SelfTestCmd::Min, SelfTestCmd::Min], Duration::from_millis(500)),
        SelfTestStep([SelfTestCmd::Min, SelfTestCmd::Max, SelfTestCmd::Min, SelfTestCmd::Def], Duration::from_millis(500)),
        SelfTestStep([SelfTestCmd::Min, SelfTestCmd::Min, SelfTestCmd::Max, SelfTestCmd::Max], Duration::from_millis(500)),
        SelfTestStep([SelfTestCmd::Def, SelfTestCmd::Def, SelfTestCmd::Def, SelfTestCmd::Def], Duration::from_millis(500)),
    ];
}

#[derive(Debug)]
pub(crate) struct SelfTestState {
    pub active: bool,
    pub progress: i64
}

#[derive(Debug)]
pub(crate) struct TestBoxState {
    pub red_led: PositionerState,
    pub yellow_led: PositionerState,
    pub green_led: PositionerState,
    pub servo: PositionerState,
    pub sensor: SensorState,
    pub self_test: SelfTestState
}

struct TestBox {
    pub red_led: Positioner,
    pub yellow_led: Positioner,
    pub green_led: Positioner,
    pub servo: Positioner,
    pub sensor: Sensor,

    next_self_test_step: time::Instant,
    self_test_stage: usize,
}

impl TestBox {
    fn new() -> Self {
        Self {
            red_led: Positioner::new(0, 1023, 0),
            yellow_led: Positioner::new(0, 1023, 0),
            green_led: Positioner::new(0, 1023, 0),
            servo: Positioner::new(0, 180, 90),
            sensor: Sensor::new(),
            next_self_test_step: time::Instant::now(),
            self_test_stage: SELF_TEST.len(),
        }
    }

    fn get(&self) -> TestBoxState {
        TestBoxState {
            red_led: self.red_led.get(),
            yellow_led: self.yellow_led.get(),
            green_led: self.green_led.get(),
            servo: self.servo.get(),
            sensor: self.sensor.get(),
            self_test: self.get_self_test(),
        }
    }

    fn do_self_test_step(&mut self, now: &time::Instant) -> bool {
        if self.self_test_stage < SELF_TEST.len() && *now > self.next_self_test_step {
            debug!("Executing self test step {}", self.self_test_stage);

            let stage = &SELF_TEST[self.self_test_stage];

            let positioners = [
                &mut self.red_led, &mut self.yellow_led, &mut self.green_led, &mut self.servo
            ];

            for (positioner, action) in zip(positioners, &stage.0) {
                let _ = match action {
                    SelfTestCmd::Min => positioner.set_min(),
                    SelfTestCmd::Max => positioner.set_max(),
                    SelfTestCmd::Def => positioner.reset()
                };
            }

            self.next_self_test_step = *now + stage.1;
            self.self_test_stage += 1;
            true
        } else {
            false
        }
    }

    fn tick(&mut self) -> bool {
        let now = time::Instant::now();
        let sensor_changed = self.sensor.update(&now);
        let self_test_changed = self.do_self_test_step(&now);

        sensor_changed || self_test_changed
    }

    fn start_self_test(&mut self) -> SelfTestState {
        if self.self_test_stage == SELF_TEST.len() {
            let now = time::Instant::now();
            self.self_test_stage = 0;
            self.next_self_test_step = now + SELF_TEST[0].1;
        }
        self.get_self_test()
    }

    fn stop_self_test(&mut self) -> SelfTestState {
        self.self_test_stage = SELF_TEST.len();
        self.get_self_test()
    }

    fn get_self_test(&self) -> SelfTestState {
        let stage = self.self_test_stage;
        let active = stage < SELF_TEST.len();
        let progress = if active { (100*stage/SELF_TEST.len()) as i64 } else { 0 };
        SelfTestState { active, progress }
    }
}

pub(crate) async fn testbox(
    mut incoming_requests: mpsc::Receiver<Request>,
    outgoing_responses: mpsc::Sender<Response>,
    state_update_tx: mpsc::Sender<TestBoxState>
) -> Result<(), Box<dyn Error>> {

    let mut tbox = TestBox::new();
    let mut interval = time::interval(time::Duration::from_millis(100));

    // Send first update
    state_update_tx.send(tbox.get()).await?;

    while select! {
        _ = interval.tick() => {
            if tbox.tick() {
                state_update_tx.send(tbox.get()).await?;
            }
            true
        }

        req = incoming_requests.recv() => {
            match req {
                Some(req) => {
                    let response = match req {
                        Request::Id => Response::Id("ESP8266_WEMOS_D1MINI".into()),

                        Request::Get(RequestNoun::RedLed) => Response::Value(tbox.red_led.get().value),
                        Request::Get(RequestNoun::YellowLed) => Response::Value(tbox.yellow_led.get().value),
                        Request::Get(RequestNoun::GreenLed) => Response::Value(tbox.green_led.get().value),
                        Request::Get(RequestNoun::Servo) => Response::Value(tbox.servo.get().value),
                        Request::Get(RequestNoun::TempAndHum) => {
                            let SensorState { status, temperature, humidity } = tbox.sensor.get();
                            Response::TempAndHum(status, temperature, humidity)
                        },
                        Request::Get(RequestNoun::SelfTest) => {
                            let SelfTestState { active, progress } = tbox.get_self_test();
                            Response::SelfTest(active, progress)
                        },

                        Request::Set(RequestNoun::RedLed, v) => Response::Value(tbox.red_led.set(v).value),
                        Request::Set(RequestNoun::YellowLed, v) => Response::Value(tbox.yellow_led.set(v).value),
                        Request::Set(RequestNoun::GreenLed, v) => Response::Value(tbox.green_led.set(v).value),
                        Request::Set(RequestNoun::Servo, v) => Response::Value(tbox.servo.set(v).value),
                        Request::Set(RequestNoun::SelfTest, v) => {
                            match v {
                                0 | 1 => {
                                    let SelfTestState { active, progress } = if v == 1 {
                                        tbox.start_self_test()
                                    } else {
                                        tbox.stop_self_test()
                                    };
                                    Response::SelfTest(active, progress)
                                },
                                _ => {
                                    Response::Error(ResponseError::BadValue)
                                }
                            }
                        },
                        Request::Set(RequestNoun::TempAndHum, _) => Response::Error(ResponseError::BadNoun),
                    };

                    outgoing_responses.send(response).await?;
                    state_update_tx.send(tbox.get()).await?;
                    true
                }

                None => {
                    info!("Incoming requests channel closed, exiting");
                    false
                }
            }
        }
    } {}

    Ok(())
}