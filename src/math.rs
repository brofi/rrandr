#[derive(PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Rect {
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}

impl Rect {
    pub fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        if i32::from(x) + i32::from(width) > 0xFFFF {
            panic!("right side needs to be smaller than u16::MAX");
        }
        if i32::from(y) + i32::from(height) > 0xFFFF {
            panic!("bottom side needs to be smaller than u16::MAX");
        }
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    pub fn x(&self) -> i16 {
        self.x
    }

    pub fn y(&self) -> i16 {
        self.y
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn bounds(rects: Vec<Rect>) -> Rect {
        rects
            .into_iter()
            .reduce(|acc, r| acc.union(&r))
            .unwrap_or_default()
    }

    fn right(&self) -> i32 {
        i32::from(self.x) + i32::from(self.width)
    }

    fn bottom(&self) -> i32 {
        i32::from(self.y) + i32::from(self.height)
    }

    pub fn center(&self) -> Point {
        Point {
            x: i32::from(self.x) + i32::from(self.width / 2),
            y: i32::from(self.y) + i32::from(self.height / 2),
        }
    }

    pub fn contains_f(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x <= f64::from(self.right())
            && y >= f64::from(self.y)
            && y <= f64::from(self.bottom())
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= i32::from(self.x) && x <= self.right() && y >= i32::from(self.y) && y <= self.bottom()
    }

    pub fn union(&self, o: &Self) -> Self {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let width = u16::try_from(self.right().max(o.right()) - x as i32).unwrap();
        let height = u16::try_from(self.bottom().max(o.bottom()) - y as i32).unwrap();
        Self::new(x, y, width, height)
    }

    pub fn intersect(&self, o: &Self) -> Option<Self> {
        let max_left = self.x.max(o.x);
        let min_right = self.right().min(o.right());
        let max_top = self.y.max(o.y);
        let min_bottom = self.bottom().min(o.bottom());

        if i32::from(max_left) >= min_right || i32::from(max_top) >= min_bottom {
            return None;
        }
        Some(Rect::new(
            max_left,
            max_top,
            u16::try_from(min_right - i32::from(max_left)).unwrap(),
            u16::try_from(min_bottom - i32::from(max_top)).unwrap(),
        ))
    }

    pub fn translate(&mut self, dx: i16, dy: i16) {
        self.x = self.x.saturating_add(dx);
        self.y = self.y.saturating_add(dy);
    }

    pub fn scale(&mut self, scale: f64) {
        self.x = (f64::from(self.x) * scale).round() as i16;
        self.y = (f64::from(self.y) * scale).round() as i16;
        self.width = (f64::from(self.width) * scale).round() as u16;
        self.height = (f64::from(self.height) * scale).round() as u16;
    }

    pub fn transform(&self, translate: [i16; 2], scale: f64) -> [f64; 4] {
        [
            f64::from(self.x.saturating_add(translate[0])) * scale,
            f64::from(self.y.saturating_add(translate[1])) * scale,
            f64::from(self.width) * scale,
            f64::from(self.height) * scale,
        ]
    }
}
