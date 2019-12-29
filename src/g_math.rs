use std::cell::RefCell;

#[derive(Default, Debug, Copy, Clone)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn intersects(&self, o: &Rect) -> bool {
        if self.width == 0 || o.width == 0 || self.height == 0 || o.height == 0 {
            return false;
        }

        o.x + (o.width as i32) > self.x
            && o.y + (o.height as i32) > self.y
            && self.x + (self.width as i32) > o.x
            && self.y + (self.height as i32) > o.y
    }

    pub fn _intersection(&self, o: &Rect) -> Option<Rect> {
        let mut i = self.clone();

        let rx1 = o.x;
        let ry1 = o.y;

        let mut ix2 = self.x + self.width as i32;
        let mut iy2 = self.y + self.height as i32;
        let ox2 = o.x + o.width as i32;
        let oy2 = o.y + o.height as i32;

        if i.x < rx1 {
            i.x = rx1;
        }
        if i.y < ry1 {
            i.y = ry1;
        }
        if ix2 > ox2 {
            ix2 = ox2;
        }
        if iy2 > oy2 {
            iy2 = oy2;
        }

        ix2 -= i.x;
        iy2 -= i.y;

        if ix2 <= 0 || iy2 <= 0 {
            return None;
        }

        i.width = ix2 as u32;
        i.height = iy2 as u32;

        Some(i)
    }

    pub fn union(&self, o: &Rect) -> Rect {
        if self.width == 0 || self.height == 0 {
            return o.clone();
        }

        let mut u = self.clone();

        if o.width == 0 || o.height == 0 {
            return u;
        }

        let mut ux2 = u.x + u.width as i32;
        let mut uy2 = u.y + u.height as i32;
        let ox2 = o.x + o.width as i32;
        let oy2 = o.y + o.height as i32;

        if u.x > o.x {
            u.x = o.x;
        }
        if u.y > o.y {
            u.y = o.y;
        }
        if ux2 < ox2 {
            ux2 = ox2;
        }
        if uy2 < oy2 {
            uy2 = oy2;
        }

        u.width = (ux2 - u.x) as u32;
        u.height = (uy2 - u.y) as u32;

        u
    }
}

#[derive(Debug, Clone)]
pub struct OutputNode {
    pub name: String,
    pub rect: Rect,
}

impl OutputNode {
    pub fn has_overlap(&self, nodes: &Vec<Self>) -> bool {
        for n in nodes {
            if self.rect.intersects(&n.rect) {
                return true;
            }
        }
        false
    }

    pub fn _get_fist_overlap(&self, nodes: &Vec<Self>) -> Option<Rect> {
        for n in nodes {
            if let Some(i) = self.rect._intersection(&n.rect) {
                return Some(i);
            }
        }
        None
    }
}

pub trait ToOutputNode {
    fn to_output_node(&self) -> OutputNode;
}

pub struct OverlapDebugInfo {
    pub step_nodes: RefCell<Vec<OutputNode>>,
}

impl OverlapDebugInfo {
    pub fn new() -> Self {
        OverlapDebugInfo {
            step_nodes: RefCell::new(Vec::new()),
        }
    }
}
