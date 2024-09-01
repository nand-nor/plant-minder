use tokio::sync::mpsc::unbounded_channel;

use pmind_broker::BrokerError;
use pmindd::{
    event::{Event, EventHandler},
    minder::{handle_app_cmd, handle_node_state_change, PlantMinder, PlantMinderResult, Tui},
    PlantMinderError,
};

use pmindb::PlantDatabaseHandler;

use tracing_appender::rolling;
use tracing_subscriber::FmtSubscriber;

use tracing_log::LogTracer;

#[actix::main]
async fn main() -> PlantMinderResult<()> {
    LogTracer::init().expect("Unable to set up log tracer");

    // TODO set up some kind of log zip / roll functionality
    let log = rolling::daily("./logs", "debug");
    let (nb, _guard) = tracing_appender::non_blocking(log);

    let sub = FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(nb)
        .finish();

    tracing::subscriber::set_global_default(sub).expect("Unable to set up tracing subscriber");

    let (client_event_tx, client_event_rx) = unbounded_channel();
    let mut app = PlantMinder::new(500, client_event_rx);

    let broker_handle = pmind_broker::broker(
        tokio::time::Duration::from_secs(15),
        500, // tick rate for broker event loop is in millis
    )
    .await
    .map_err(|e| {
        log::error!("Error creating broker & handle {e:}");
        PlantMinderError::BrokerError(e)
    })?;

    let (sensor_stream_tx, sensor_stream_rx) = unbounded_channel();
    let (node_state_tx, node_state_rx) = unbounded_channel();

    let mut events = EventHandler::new(1, sensor_stream_rx, node_state_rx, client_event_tx);

    // Subscribe to all node sensor related events
    broker_handle
        .send(pmind_broker::ClientSubscribe {
            id: 0,
            sensor_readings: sensor_stream_tx,
            node_status: node_state_tx,
        })
        .await
        .map_err(|e| {
            log::error!("Error sending client subscribe request {e:}");
            PlantMinderError::BrokerError(BrokerError::ActorError)
        })??;

    #[cfg(feature = "database")]
    {
        let (mut db_handle, db_stream_tx, db_state_tx) =
            PlantDatabaseHandler::new_with_db_conn_tasks("file:./plantminder.db").await?;

        // Set up database subscription to all node sensor related events
        broker_handle
            .send(pmind_broker::ClientSubscribe {
                id: 1,
                sensor_readings: db_stream_tx,
                node_status: db_state_tx,
            })
            .await
            .map_err(|e| {
                log::error!("Error sending database subscribe request {e:}");
                PlantMinderError::BrokerError(BrokerError::ActorError)
            })??;

        app.enable_database(db_handle)?;
    }

    let mut tui = Tui::new()?;
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;
        match events.next().await {
            Ok(Event::Tick) => app.tick().await,
            Ok(Event::AppCmd(cmd)) => handle_app_cmd(cmd, &mut app).await,
            Ok(Event::NodeState(status)) => handle_node_state_change(status, &mut app).await,
            Err(e) => {
                log::error!("Error in app event loop {e:}, exiting");
                break;
            }
        }
    }

    tui.exit()?;

    Ok(())
}
