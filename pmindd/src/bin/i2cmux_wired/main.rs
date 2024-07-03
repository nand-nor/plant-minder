use linux_embedded_hal::I2cdev;

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use xca9548a::{I2cSlave, SlaveAddr, Xca9548a};

use pmindp_sensor::{SoilSensorError, ATSAMD10};

#[tokio::main]
async fn main() -> Result<(), SoilSensorError> {
    let dev = I2cdev::new("/dev/i2c-1")?;
    let address = SlaveAddr::default();

    let i2c_switch: &'static mut _ = Box::leak(Box::new(Xca9548a::new(dev, address)));
    let parts = i2c_switch.split();

    let sensors = [
        Arc::new(Mutex::new(ATSAMD10::new(parts.i2c0, 0x36, 200, 5000))),
        Arc::new(Mutex::new(ATSAMD10::new(parts.i2c1, 0x36, 200, 5000))),
        Arc::new(Mutex::new(ATSAMD10::new(parts.i2c2, 0x36, 200, 5000))),
        Arc::new(Mutex::new(ATSAMD10::new(parts.i2c3, 0x36, 200, 5000))),
    ];

    loop {
        sensors
            .iter()
            .enumerate()
            .map(|(i, sensor)| async move {
                read_from_sensor(Arc::clone(sensor), format!("Plant Sensor {i:}")).await?;
                Ok::<(), SoilSensorError>(())
            })
            .for_each(|_|{}); 

        sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

async fn read_from_sensor(
    sensor: Arc<Mutex<ATSAMD10<I2cSlave<'static, Xca9548a<I2cdev>, I2cdev>>>>,
    title: String,
) -> Result<(), SoilSensorError> {
    if let Err(_e) = tokio::spawn(async move {
        let mut guard = sensor.lock().await;
        let temp = guard.temperature().await?;
        let moist = guard.moisture().await?;
        println!("Plant sensor {title} reading: {temp:?} {moist:?}");
        Ok::<(), SoilSensorError>(())
    })
    .await
    {
        // to do log error
    }
    Ok(())
}
