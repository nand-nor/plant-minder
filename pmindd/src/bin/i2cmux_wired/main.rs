use linux_embedded_hal::I2cdev;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use xca9548a::{SlaveAddr, Xca9548a, I2cSlave};
use pmindd;

use pmindp_sensor::{*, ATSAMD10, Sensor as SoilSensor, SoilSensorError};


#[tokio::main]
async fn main() -> Result<(), SoilSensorError> {
    let dev = I2cdev::new("/dev/i2c-1")?;
    let address = SlaveAddr::default();

    let i2c_switch: &'static mut _ = Box::leak(Box::new(Xca9548a::new(dev, address)));
    let parts = i2c_switch.split();

    let sensor_0 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c0, 0x36, 200, 5000,)));
    let sensor_1 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c1, 0x36, 200, 5000,)));
    let sensor_2 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c2, 0x36, 200, 5000,)));
    let sensor_3 = Arc::new(Mutex::new(SoilSensor::new(parts.i2c3, 0x36, 200, 5000,)));

    loop {
        read_from_sensor(Arc::clone(&sensor_0), "Plant Sensor 0".to_string()).await?;
        read_from_sensor(Arc::clone(&sensor_1), "Plant Sensor 1".to_string()).await?;
        read_from_sensor(Arc::clone(&sensor_2), "Plant Sensor 2".to_string()).await?;
        read_from_sensor(Arc::clone(&sensor_3), "Plant Sensor 3".to_string()).await?;
        sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

async fn read_from_sensor(
    sensor: Arc<Mutex<SoilSensor<ATSAMD10<I2cSlave<'static, Xca9548a<I2cdev>, I2cdev>>>>>,
    title: String,
) -> Result<(), SoilSensorError> {
    if let Err(e) = tokio::spawn(async move {
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
