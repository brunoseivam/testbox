use std::{net::SocketAddr, error::Error};

use log::info;
use tokio::{net::TcpListener, io::AsyncReadExt, io::AsyncWriteExt, sync::mpsc, select};

pub(crate) async fn server<const LEN: usize>(
    port: u16,
    incoming: mpsc::Sender<Option<Vec<u8>>>,
    mut outgoing: mpsc::Receiver<Vec<u8>>
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;
    info!("Listening on {}", listener.local_addr()?);

    loop {
        let (mut stream, remote_addr) = listener.accept().await?;
        info!("New connection from {}", remote_addr);

        let mut buffer = [0u8; LEN];

        while select! {
            response = outgoing.recv() => {
                match response {
                    Some(r) => {
                        stream.write_all(&r).await?;
                        true
                    },
                    None => {
                        info!("Outgoing channel is closed. Exiting");
                        false
                    }
                }
            }

            request = stream.read(&mut buffer) => {
                match request? {
                    0 => {
                        info!("Got 0 bytes, closing the connection");
                        incoming.send(None).await?;
                        false
                    }
                    n => {
                        incoming.send(Some(buffer[..n].to_vec())).await?;
                        true
                    },
                }
            }
        } {}
    }
}
