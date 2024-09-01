#[actix::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    log::info!("Initializing broker");

    let broker_handle = pmind_broker::broker(tokio::time::Duration::from_secs(15), 500)
        .await
        .map_err(|e| {
            log::error!("Error creating broker & handle {e:}");
            e
        })?;

    // All received data will be dropped because no client is polling the recv streams
    // but this is OK for testing just the broker layer
    let (sensor_stream_tx, _sensor_stream_rx) = tokio::sync::mpsc::unbounded_channel();
    let (node_state_tx, _node_state_rx) = tokio::sync::mpsc::unbounded_channel();

    broker_handle
        .send(pmind_broker::ClientSubscribe {
            id: 0,
            sensor_readings: sensor_stream_tx,
            node_status: node_state_tx,
        })
        .await
        .map_err(|e| {
            log::error!("Error sending client subscribe request {e:}");
            e
        })??;

    loop {
        // todo block on broker or SIGINT / SIGQUIT / SIGSTP from stdin
    }

    Ok(())
}
