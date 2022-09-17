use std::{error::Error};

use log::info;
use tokio::{sync::mpsc, signal};

mod server;
mod parser;
mod testbox;
mod ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let (incoming_tx, incoming_rx) = mpsc::channel(10);
    let (outgoing_tx, outgoing_rx) = mpsc::channel(10);

    let (requests_tx, requests_rx) = mpsc::channel(10);
    let (responses_tx, responses_rx) = mpsc::channel(10);

    let (ui_tx, ui_rx) = mpsc::channel(10);

    tokio::spawn(async move {
        server::server::<256usize>(12345, incoming_tx, outgoing_rx).await.unwrap()
    });

    tokio::spawn(async move {
        parser::parser::<256usize>(incoming_rx, outgoing_tx, requests_tx, responses_rx).await.unwrap()
    });

    tokio::spawn(async move {
        testbox::testbox(requests_rx, responses_tx, ui_tx).await.unwrap()
    });

    tokio::spawn(async move {
        ui::ui(ui_rx).await.unwrap()
    });

    // Wait for CTRL+C
    signal::ctrl_c().await.expect("Failed to listen to CTRL+C");

    info!("Received CTRL+C, exiting...");

    Ok(())
}