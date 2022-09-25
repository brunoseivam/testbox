use std::{error::Error, fmt, sync::Mutex};

use status_line::StatusLine;
use tokio::sync::mpsc;

use crate::testbox::TestBoxState;

struct Status(Mutex<TestBoxState>);

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tb = self.0.lock().expect("Failed to acquire lock");
        write!(
            f, "RED: {:4}   YLW: {:4}   GRN: {:4}  Servo: {:3}  TEMP: {:2.2}  HUM: {:2.2}  SlfTst:{:3}%",
            tb.red_led.value, tb.yellow_led.value, tb.green_led.value,
            tb.servo.value, tb.sensor.temperature, tb.sensor.humidity,
            tb.self_test.progress

        )
    }
}

pub(crate) async fn ui(
    mut state_update_rx: mpsc::Receiver<TestBoxState>
) -> Result<(), Box<dyn Error>> {

    let status = StatusLine::new(
        Status(
            Mutex::new(
                state_update_rx.recv().await.ok_or("Failed to receive first update")?
            )
        )
    );

    while let Some(update) = state_update_rx.recv().await {
        let mut inner_status = status.0.lock().expect("Failed to acquire data");
        *inner_status = update;
    }

    Ok(())
}