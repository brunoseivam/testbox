use std::{error::Error, convert::{TryFrom, TryInto}, collections::HashSet};

use log::{info, debug};
use tokio::{sync::mpsc, select};
use regex::bytes::Regex;
use lazy_static::lazy_static;

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum RequestNoun {
    RedLed,
    YellowLed,
    GreenLed,
    Servo,
    TempAndHum,
    SelfTest
}

impl TryFrom<&[u8]> for RequestNoun {
    type Error = ResponseError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        match data {
            b"RED_LED" => Ok(Self::RedLed),
            b"YELLOW_LED" => Ok(Self::YellowLed),
            b"GREEN_LED" => Ok(Self::GreenLed),
            b"SERVO" => Ok(Self::Servo),
            b"TEMP_AND_HUM" => Ok(Self::TempAndHum),
            b"SELF_TEST" => Ok(Self::SelfTest),
            _ => Err(ResponseError::BadNoun)
        }
    }
}

lazy_static! {
    static ref SETTABLE: HashSet<RequestNoun> = HashSet::from([
        RequestNoun::RedLed, RequestNoun::YellowLed, RequestNoun::GreenLed,
        RequestNoun::Servo, RequestNoun::SelfTest
    ]);

    static ref GETTABLE: HashSet<RequestNoun> = HashSet::from([
        RequestNoun::RedLed, RequestNoun::YellowLed, RequestNoun::GreenLed,
        RequestNoun::Servo, RequestNoun::TempAndHum, RequestNoun::SelfTest
    ]);
}

#[derive(Debug)]
pub enum Request {
    Id,
    Get(RequestNoun),
    Set(RequestNoun, i64)
}

impl TryFrom<&[u8]> for Request {
    type Error = ResponseError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {

        let re = Regex::new(r"([^ \r\n]+)( [^ \r\n]+)?( [^\r\n]+)?\r?\n")
            .expect("Failed to create decoder regex");

        let caps = re.captures(data).ok_or(ResponseError::BadSyntax)?;

        debug!("verb={:?} noun={:?} value={:?}",
            caps.get(1).map_or("".into(), |v| String::from_utf8_lossy(v.as_bytes())),
            caps.get(2).map_or("".into(), |v| String::from_utf8_lossy(v.as_bytes())),
            caps.get(3).map_or("".into(), |v| String::from_utf8_lossy(v.as_bytes())));

        let verb = caps.get(1).ok_or(ResponseError::BadSyntax)?;

        match verb.as_bytes() {
            b"ID" => caps.get(2).map_or(Ok(Self::Id), |_| Err(ResponseError::BadNoun)),

            verb @ (b"GET" | b"SET") => {
                let noun: RequestNoun = caps.get(2)
                    .ok_or(ResponseError::BadNoun)?
                    .as_bytes()[1..] // skip leading space
                    .try_into()?;

                if verb == b"GET" {
                    GETTABLE.get(&noun).ok_or(ResponseError::BadNoun)?;
                    caps.get(3).map_or(Ok(Self::Get(noun)), |_| Err(ResponseError::BadValue))
                } else {
                    SETTABLE.get(&noun).ok_or(ResponseError::BadNoun)?;
                    let value = caps.get(3).ok_or(ResponseError::BadValue)?;
                    let value = String::from_utf8_lossy(&value.as_bytes()[1..]); // skip leading space
                    let value = value.parse::<i64>().map_err(|_| ResponseError::BadValue)?;

                    Ok(Self::Set(noun, value))
                }
            }

            _ => {
                Err(ResponseError::BadVerb)
            }
        }
    }
}

#[derive(Debug)]
pub enum ResponseError {
    BadSyntax,
    BadVerb,
    BadNoun,
    BadValue
}

impl From<ResponseError> for &'static str {
    fn from(e: ResponseError) -> Self {
        match e {
            ResponseError::BadSyntax => "BAD_SYNTAX",
            ResponseError::BadVerb => "BAD_VERB",
            ResponseError::BadNoun => "BAD_NOUN",
            ResponseError::BadValue => "BAD_VALUE",
        }
    }
}

#[derive(Debug)]
pub enum Response {
    Id(String),
    Value(i64),
    TempAndHum(String, f64, f64),
    SelfTest(bool, i64),
    Error(ResponseError)
}

impl From<Response> for Vec<u8> {
    fn from(r: Response) -> Self {
        match r {
            Response::Id(id) => format!("OK {}\r\n", id),
            Response::Value(v) => format!("OK {}\r\n", v),
            Response::TempAndHum(s, t, h) => format!("OK {} {:.2} {:.2}\r\n", s, t, h),
            Response::SelfTest(a, p) => format!("OK {} {}\r\n", if a {1} else {0}, p),
            Response::Error(e) => {
                let e: &'static str = e.into();
                format!("ERR {}\r\n", e)
            }
        }.into()
    }
}

pub(crate) async fn parser<const LEN:usize> (
    mut incoming_bytes: mpsc::Receiver<Option<Vec<u8>>>,
    outgoing_bytes: mpsc::Sender<Vec<u8>>,
    incoming_requests: mpsc::Sender<Request>,
    mut outgoing_responses: mpsc::Receiver<Response>
) -> Result<(), Box<dyn Error>> {

    let mut buffer = [0u8; LEN];
    let mut buffer_len = 0usize;

    while select! {
        ib = incoming_bytes.recv() => {
            match ib {
                Some(None) => {
                    info!("Client disconnected, clearing buffer");
                    buffer_len = 0;
                    true
                },
                Some(Some(ib)) => {
                    info!("Received {} bytes to be parsed {:?}", ib.len(), String::from_utf8_lossy(&ib));

                    for c in ib {
                        buffer[buffer_len] = c;
                        buffer_len += 1;

                        if c == b'\n' || buffer_len == LEN {
                            match (&buffer[..buffer_len]).try_into() {
                                Ok(r) => {
                                    info!("{:?}", r);
                                    incoming_requests.send(r).await?;
                                }
                                Err(e) => {
                                    outgoing_bytes.send(Response::Error(e).into()).await?
                                }
                            }

                            buffer_len = 0;
                        }
                    }
                    true
                },
                None => {
                    info!("Receiving channel for bytes is closed, exiting");
                    false
                }
            }
        }

        or = outgoing_responses.recv() => {
            match or {
                Some(r) => {
                    let r: Vec<u8> = r.into();
                    info!("Sending response {:?}", String::from_utf8_lossy(&r));
                    outgoing_bytes.send(r).await?;
                    true
                }

                None => {
                    info!("Receiving channel for responses is closed, exiting");
                    false
                }
            }
        }
    } {}

    Ok(())
}
