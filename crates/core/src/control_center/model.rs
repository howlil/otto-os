pub const UPDATE_CHECKING_LABEL: &str = "Checking...";
pub const SLIDER_KEY_STEP_PERCENT: i8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlCenterPage {
    #[default]
    Main,
    Wifi,
    Bluetooth,
    Sound,
    Display,
    Gallery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BluetoothDeviceKind {
    Headphones,
    Speaker,
    Mouse,
    Keyboard,
    Phone,
    #[default]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal_percent: u8,
    pub secured: bool,
    pub connected: bool,
    pub saved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WifiState {
    pub supported: bool,
    pub enabled: bool,
    pub busy: bool,
    pub connected_ssid: Option<String>,
    pub networks: Vec<WifiNetwork>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BluetoothDevice {
    pub id: String,
    pub name: String,
    pub kind: BluetoothDeviceKind,
    pub connected: bool,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BluetoothState {
    pub supported: bool,
    pub enabled: bool,
    pub busy: bool,
    pub devices: Vec<BluetoothDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}
