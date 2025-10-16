use alloc::boxed::Box;

#[allow(unused_imports)]
use crate::prelude::*;

/// The unified type of the NIC devices.
#[cfg(feature = "net")]
pub type AxNetDevice = Box<dyn NetDriverOps>;
/// The unified type of the block storage devices.
#[cfg(feature = "block")]
pub type AxBlockDevice = Box<dyn BlockDriverOps>;
/// The unified type of the graphics display devices.
#[cfg(feature = "display")]
pub type AxDisplayDevice = Box<dyn DisplayDriverOps>;
/// The unified type of the input devices.
#[cfg(feature = "input")]
pub type AxInputDevice = Box<dyn InputDriverOps>;

/// The unified type of the socket devices.
#[cfg(feature = "vsock")]
pub type AxVsockDevice = Box<dyn VsockDriverOps>;

impl super::AxDeviceEnum {
    /// Constructs a network device.
    #[cfg(feature = "net")]
    pub fn from_net(dev: impl NetDriverOps + 'static) -> Self {
        Self::Net(Box::new(dev))
    }

    /// Constructs a block device.
    #[cfg(feature = "block")]
    pub fn from_block(dev: impl BlockDriverOps + 'static) -> Self {
        Self::Block(Box::new(dev))
    }

    /// Constructs a display device.
    #[cfg(feature = "display")]
    pub fn from_display(dev: impl DisplayDriverOps + 'static) -> Self {
        Self::Display(Box::new(dev))
    }

    /// Constructs an input device.
    #[cfg(feature = "input")]
    pub fn from_input(dev: impl InputDriverOps + 'static) -> Self {
        Self::Input(Box::new(dev))
    }
}
