use std::ops::Deref;

use cairo::{ffi, Error, Surface, SurfaceType, XCBConnection, XCBDrawable, XCBVisualType};

// Copied from cairo to implement Send and Sync for XCBSurface

#[derive(Debug)]
#[repr(transparent)]
pub struct XCBSurfaceS(Surface);

impl TryFrom<Surface> for XCBSurfaceS {
    type Error = Surface;

    #[inline]
    fn try_from(surface: Surface) -> Result<XCBSurfaceS, Surface> {
        if surface.type_() == SurfaceType::Xcb { Ok(XCBSurfaceS(surface)) } else { Err(surface) }
    }
}

impl XCBSurfaceS {
    #[inline]
    pub unsafe fn from_raw_full(ptr: *mut ffi::cairo_surface_t) -> Result<XCBSurfaceS, Error> {
        let surface = Surface::from_raw_full(ptr)?;
        Self::try_from(surface).map_err(|_| Error::SurfaceTypeMismatch)
    }
}

impl Deref for XCBSurfaceS {
    type Target = Surface;

    #[inline]
    fn deref(&self) -> &Surface { &self.0 }
}

impl AsRef<Surface> for XCBSurfaceS {
    #[inline]
    fn as_ref(&self) -> &Surface { &self.0 }
}

impl Clone for XCBSurfaceS {
    #[inline]
    fn clone(&self) -> XCBSurfaceS { XCBSurfaceS(self.0.clone()) }
}

impl XCBSurfaceS {
    pub fn create(
        connection: &XCBConnection,
        drawable: &XCBDrawable,
        visual: &XCBVisualType,
        width: i32,
        height: i32,
    ) -> Result<Self, Error> {
        unsafe {
            Self::from_raw_full(ffi::cairo_xcb_surface_create(
                connection.to_raw_none(),
                drawable.0,
                visual.to_raw_none(),
                width,
                height,
            ))
        }
    }
}

unsafe impl Send for XCBSurfaceS {}
unsafe impl Sync for XCBSurfaceS {}
