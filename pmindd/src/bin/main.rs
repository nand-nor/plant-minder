use tokio::sync::mpsc::unbounded_channel;

use pmindb::BrokerCoordinator;
use pmindd::{
    event::{handle_app_cmd, handle_node_reg_task, handle_sensor_stream_task, Event, EventHandler},
    minder::{PlantMinder, PlantMinderResult, Tui},
};
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

    let (stream_tx, stream_rx) = unbounded_channel();
    let (registration_tx, registration_rx) = unbounded_channel();

    let mut broker = BrokerCoordinator::new_no_db_con(
        stream_tx,
        registration_tx,
        tokio::time::Duration::from_secs(15),
    )
    .await?;
    let mut app = PlantMinder::new(500);

    tokio::spawn(async move {
        broker.exec_monitor().await;
    });

    let mut events = EventHandler::new(1, stream_rx, registration_rx);
    let mut tui = Tui::new()?;
    tui.init()?;

    while app.running {
        tui.draw(&mut app)?;
        match events.next().await {
            Ok(Event::Tick) => app.tick().await,
            Ok(Event::AppCmd(cmd)) => handle_app_cmd(cmd, &mut app).await,
            Ok(Event::SensorNodeEvent(r)) => {
                handle_sensor_stream_task(&mut app, r).await;
            }
            Ok(Event::NodeRegistration(n)) => {
                handle_node_reg_task(&mut app, n).await;
            }
            Err(e) => {
                log::error!("Error in app event loop {e:}, exiting");
                break;
            }
        }
    }

    tui.exit()?;

    Ok(())
}
