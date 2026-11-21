//! Acceso a la tarjeta: volumen y mux por el mixer simple, jack por el control CARD.

use std::ffi::CString;

use alsa::ctl::{Ctl, ElemId, ElemIface, ElemType, ElemValue};
use alsa::mixer::{Selem, SelemChannelId, SelemId};

use crate::volume::VolumeScale;

pub type Result<T> = std::result::Result<T, String>;

pub struct Mixer {
    mixer: alsa::Mixer,
    volume: String,
}

impl Mixer {
    pub fn open(ctl_name: &str, volume: &str) -> Result<Mixer> {
        // No bloqueante: `handle_events` solo vacía lo pendiente.
        let mixer = alsa::Mixer::new(ctl_name, true).map_err(|e| format!("{ctl_name}: {e}"))?;
        let this = Mixer {
            mixer,
            volume: volume.into(),
        };
        let selem = this.selem(volume)?;
        if !selem.has_playback_volume() {
            return Err(format!("{volume} no tiene volumen de reproducción"));
        }
        Ok(this)
    }

    fn selem(&self, name: &str) -> Result<Selem<'_>> {
        self.mixer
            .find_selem(&SelemId::new(name, 0))
            .ok_or_else(|| format!("no existe el control {name:?}"))
    }

    /// Trae al caché del mixer los cambios hechos por otros (amixer, otras apps).
    fn refresh(&self) {
        let _ = self.mixer.handle_events();
    }

    pub fn volume_scale(&self) -> Result<VolumeScale> {
        let selem = self.selem(&self.volume)?;
        let (raw_min, raw_max) = selem.get_playback_volume_range();
        let (db_min, db_max) = selem.get_playback_db_range();
        Ok(VolumeScale {
            raw_min,
            raw_max,
            db_min: db_min.to_db() as f64,
            db_max: db_max.to_db() as f64,
        })
    }

    pub fn volume(&self) -> Result<i64> {
        self.refresh();
        self.selem(&self.volume)?
            .get_playback_volume(SelemChannelId::FrontLeft)
            .map_err(|e| format!("leer {}: {e}", self.volume))
    }

    /// Escribe solo la reproducción (no toca `Master Capture Volume`).
    pub fn set_volume(&self, raw: i64) -> Result<()> {
        self.selem(&self.volume)?
            .set_playback_volume_all(raw)
            .map_err(|e| format!("escribir {}: {e}", self.volume))
    }

    /// Fuerza el ítem `item` del control enumerado `name`. Devuelve si hubo que cambiarlo.
    pub fn force_enum(&self, name: &str, item: &str) -> Result<bool> {
        self.refresh();
        let selem = self.selem(name)?;
        if !selem.is_enumerated() {
            return Err(format!("{name} no es enumerado"));
        }
        let names: Vec<String> = selem
            .iter_enum()
            .map_err(|e| format!("{name}: {e}"))?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| format!("{name}: {e}"))?;
        let index = names
            .iter()
            .position(|n| n == item)
            .ok_or_else(|| format!("{name} no tiene el ítem {item:?} (hay {names:?})"))?
            as u32;
        let current = selem
            .get_enum_item(SelemChannelId::mono())
            .map_err(|e| format!("leer {name}: {e}"))?;
        if current == index {
            return Ok(false);
        }
        // Los enumerados del RK817 son mono; si fuera estéreo, se escriben los dos canales.
        for channel in [SelemChannelId::FrontLeft, SelemChannelId::FrontRight] {
            if selem.get_enum_item(channel).is_ok() {
                selem
                    .set_enum_item(channel, index)
                    .map_err(|e| format!("escribir {name}: {e}"))?;
            }
        }
        Ok(true)
    }
}

/// Control booleano del jack, con suscripción a sus cambios.
pub struct Jack {
    ctl: Ctl,
    id: ElemId,
    name: String,
}

impl Jack {
    /// Abre el control en modo bloqueante: `wait_change` duerme hasta el próximo evento.
    pub fn open(ctl_name: &str, control: &str) -> Result<Jack> {
        let ctl = Ctl::new(ctl_name, false).map_err(|e| format!("{ctl_name}: {e}"))?;
        let mut id = ElemId::new(ElemIface::Card);
        let cname = CString::new(control).map_err(|e| e.to_string())?;
        id.set_name(&cname);
        ctl.subscribe_events(true)
            .map_err(|e| format!("suscribirse a {ctl_name}: {e}"))?;
        let jack = Jack {
            ctl,
            id,
            name: control.into(),
        };
        jack.read()?;
        Ok(jack)
    }

    pub fn read(&self) -> Result<bool> {
        let mut value = ElemValue::new(ElemType::Boolean).map_err(|e| e.to_string())?;
        value.set_id(&self.id);
        self.ctl
            .elem_read(&mut value)
            .map_err(|e| format!("leer {:?}: {e}", self.name))?;
        value
            .get_boolean(0)
            .ok_or_else(|| format!("{:?} no es booleano", self.name))
    }

    /// Bloquea hasta un cambio de valor del control del jack y devuelve el valor nuevo.
    pub fn wait_change(&self) -> Result<bool> {
        loop {
            let Some(event) = self.ctl.read().map_err(|e| format!("eventos: {e}"))? else {
                continue;
            };
            let mask = event.get_mask();
            let id = event.get_id();
            if mask.value()
                && id.get_interface() == ElemIface::Card
                && id.get_name().is_ok_and(|n| n == self.name)
            {
                return self.read();
            }
        }
    }
}
