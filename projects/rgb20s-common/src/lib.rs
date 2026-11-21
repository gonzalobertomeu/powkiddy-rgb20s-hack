//! Piezas compartidas por las apps de la RGB20S (launcher, groovebox) y el servicio de audio.
//!
//! - [`audio_state`]: el estado que publica `rgb20s-audio` (volumen y modo) y su formato.
//! - [`watch`]: detección barata de cambios en ese archivo, para el bucle de una app.
//! - [`battery`]: carga de la batería desde sysfs.
//!
//! Sin hilos ni SDL: todo se llama desde el bucle de la interfaz.

pub mod audio_state;
pub mod battery;
pub mod watch;
