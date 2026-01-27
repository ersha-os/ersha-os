use tokio::{
    io,
    io::AsyncReadExt,
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

use std::sync::atomic::{AtomicU32, Ordering};

use ersha_edge::{H3Cell, ReadingPacket, transport::{Msg, MsgType, PACKET_PREAMBLE}};

static NEXT_DEVICE_ID: AtomicU32 = AtomicU32::new(1);

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr = "0.0.0.0:9001";
    let listener = TcpListener::bind(addr).await?;

    println!("Listening on {:?}...", addr);

    while let Ok((stream, _)) = listener.accept().await {

        tokio::spawn(async move {
            if let Err(e) = handle_client(stream).await {
                eprintln!("Client error: {:?}", e);
            }
        });
    }

    Ok(())
}

async fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    println!("Client connected: {:?}", stream.peer_addr()?);

    let mut hello = [0u8; 5];
    stream.read_exact(&mut hello).await?;

    if &hello != b"HELLO" {
        println!("Invalid handshake");
        return Ok(());
    }

    let mut location = [0u8; 8];
    stream.read_exact(&mut location).await?;

    let location: H3Cell = u64::from_be_bytes(location);

    let device_id = NEXT_DEVICE_ID.fetch_add(1, Ordering::Relaxed);
    stream.write_all(&device_id.to_be_bytes()).await?;
    println!("Assigned device_id={}", device_id);

    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut tmp = [0u8; 256];

    loop {
        let n = match stream.read(&mut tmp).await {
            Ok(0) => {
                println!("Device {} disconnected", device_id);
                break;
            }
            Ok(n) => n,
            Err(e) => {
                println!("Read error from device {}: {}", device_id, e);
                break;
            }
        };

        buf.extend_from_slice(&tmp[..n]);

        loop {
            let (msg, rest) = match postcard::take_from_bytes::<Msg>(&buf) {
                Ok(v) => v,
                Err(postcard::Error::DeserializeUnexpectedEnd) => {
                    // println!("Device sent incomplete bytes");
                    break;
                }
                Err(e) => {
                    println!("Malformed message from device {}: {:?}", device_id, e);
                    return Ok(());
                }
            };

            {
                if msg.preamble != PACKET_PREAMBLE {
                    println!("Invalid preamble from device {}", device_id);
                    return Ok(());
                }

                println!(
                    "[device {}] received {:?} message ({} bytes payload)",
                    device_id,
                    msg.msg_type,
                    msg.payload.len()
                );

                match msg.msg_type {
                    MsgType::Reading => {
                        let packet: ReadingPacket = match postcard::from_bytes(msg.payload) {
                            Ok(p) => p,
                            Err(_) => {
                                println!("Invalid reading payload from device {}", device_id);
                                continue;
                            }
                        };


                        println!(
                            "[device {} location {}] sensor {} reading {} => {:?}",
                            packet.device_id,
                            location,
                            packet.sensor_id,
                            packet.reading_id,
                            packet.metric
                        );
                    }
                }
            }

            buf = rest.to_vec();
        }
    }

    Ok(())
}
