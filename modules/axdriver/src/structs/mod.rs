use core::ops::{Deref, DerefMut};

use axdriver_base::{BaseDriverOps, DeviceType};
use smallvec::SmallVec;

#[cfg_attr(feature = "dyn", path = "dyn.rs")]
#[cfg_attr(not(feature = "dyn"), path = "static.rs")]
mod imp;
pub use imp::*;

/// A unified enum that represents different categories of devices.
#[allow(clippy::large_enum_variant)]
pub enum AxDeviceEnum {
    /// Network card device.
    #[cfg(feature = "net")]
    Net(AxNetDevice),
    /// Block storage device.
    #[cfg(feature = "block")]
    Block(AxBlockDevice),
    /// Graphic display device.
    #[cfg(feature = "display")]
    Display(AxDisplayDevice),
    /// Graphic input device.
    #[cfg(feature = "input")]
    Input(AxInputDevice),
    #[cfg(feature = "vsock")]
    Vsock(AxVsockDevice),
}

impl BaseDriverOps for AxDeviceEnum {
    #[inline]
    #[allow(unreachable_patterns)]
    fn device_type(&self) -> DeviceType {
        match self {
            #[cfg(feature = "net")]
            Self::Net(_) => DeviceType::Net,
            #[cfg(feature = "block")]
            Self::Block(_) => DeviceType::Block,
            #[cfg(feature = "display")]
            Self::Display(_) => DeviceType::Display,
            #[cfg(feature = "input")]
            Self::Input(_) => DeviceType::Input,
            #[cfg(feature = "vsock")]
            Self::Vsock(_) => DeviceType::Vsock,
            _ => unreachable!(),
        }
    }

    #[inline]
    #[allow(unreachable_patterns)]
    fn device_name(&self) -> &str {
        match self {
            #[cfg(feature = "net")]
            Self::Net(dev) => dev.device_name(),
            #[cfg(feature = "block")]
            Self::Block(dev) => dev.device_name(),
            #[cfg(feature = "display")]
            Self::Display(dev) => dev.device_name(),
            #[cfg(feature = "input")]
            Self::Input(dev) => dev.device_name(),
            #[cfg(feature = "vsock")]
            Self::Vsock(dev) => dev.device_name(),
            _ => unreachable!(),
        }
    }
}

/// A structure that contains all device drivers of a certain category.
pub struct AxDeviceContainer<D>(SmallVec<[D; 2]>);

impl<D> AxDeviceContainer<D> {
    /// Takes one device out of the container (will remove it from the
    /// container).
    pub fn take_one(&mut self) -> Option<D> {
        self.0.pop()
    }
}

impl<D> Deref for AxDeviceContainer<D> {
    type Target = SmallVec<[D; 2]>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<D> DerefMut for AxDeviceContainer<D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<D> Default for AxDeviceContainer<D> {
    fn default() -> Self {
        Self(Default::default())
    }
}
