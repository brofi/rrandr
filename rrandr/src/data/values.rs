use glib::ValueDelegate;
use gtk::glib;

#[derive(ValueDelegate, Clone, Copy, Default)]
#[value_delegate(from = i32)]
pub struct I16(i16);

impl I16 {
    pub fn new(value: i16) -> Self { Self(value) }

    pub fn get(self) -> i16 { self.0 }
}

impl From<i16> for I16 {
    fn from(value: i16) -> Self { Self(value) }
}

impl From<i32> for I16 {
    fn from(value: i32) -> Self { Self(value.try_into().expect("smaller value")) }
}

impl<'a> From<&'a I16> for i32 {
    fn from(value: &'a I16) -> Self { value.0.into() }
}

impl From<I16> for i32 {
    fn from(value: I16) -> Self { value.0.into() }
}

#[derive(ValueDelegate, Clone, Copy, Default)]
#[value_delegate(from = u32)]
pub struct U16(u16);

impl U16 {
    pub fn new(value: u16) -> Self { Self(value) }

    pub fn get(self) -> u16 { self.0 }
}

impl From<u16> for U16 {
    fn from(value: u16) -> Self { Self(value) }
}

impl From<u32> for U16 {
    fn from(value: u32) -> Self { Self(value.try_into().expect("smaller value")) }
}

impl<'a> From<&'a U16> for u32 {
    fn from(value: &'a U16) -> Self { value.0.into() }
}

impl From<U16> for u32 {
    fn from(value: U16) -> Self { value.0.into() }
}
