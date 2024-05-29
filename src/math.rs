#[derive(Default, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn max() -> Self { Point { x: i32::MAX, y: i32::MAX } }
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
        assert!(
            i32::from(x) + i32::from(width) <= 0xFFFF,
            "right side needs to be smaller than u16::MAX"
        );
        assert!(
            i32::from(y) + i32::from(height) <= 0xFFFF,
            "bottom side needs to be smaller than u16::MAX"
        );
        Rect { x, y, width, height }
    }

    pub fn x(&self) -> i16 { self.x }

    pub fn y(&self) -> i16 { self.y }

    pub fn width(&self) -> u16 { self.width }

    pub fn height(&self) -> u16 { self.height }

    pub fn bounds(rects: Vec<Rect>) -> Rect {
        rects.into_iter().reduce(|acc, r| acc.union(&r)).unwrap_or_default()
    }

    pub fn left(&self) -> i32 { i32::from(self.x) }

    pub fn right(&self) -> i32 { self.left() + i32::from(self.width) }

    pub fn top(&self) -> i32 { i32::from(self.y) }

    pub fn bottom(&self) -> i32 { self.top() + i32::from(self.height) }

    pub fn center(&self) -> Point {
        Point {
            x: i32::from(self.x) + i32::from(self.width / 2),
            y: i32::from(self.y) + i32::from(self.height / 2),
        }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x <= f64::from(self.right())
            && y >= f64::from(self.y)
            && y <= f64::from(self.bottom())
    }

    pub fn union(&self, o: &Self) -> Self {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let width = u16::try_from(self.right().max(o.right()) - i32::from(x)).unwrap();
        let height = u16::try_from(self.bottom().max(o.bottom()) - i32::from(y)).unwrap();
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

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    pub fn scale(&mut self, scale: f64) {
        self.x = (f64::from(self.x) * scale).round() as i16;
        self.y = (f64::from(self.y) * scale).round() as i16;
        self.width = (f64::from(self.width) * scale).round() as u16;
        self.height = (f64::from(self.height) * scale).round() as u16;
    }

    pub fn transform(&self, scale: f64, translate: [i16; 2]) -> [f64; 4] {
        [
            f64::from(self.x) * scale + f64::from(translate[0]),
            f64::from(self.y) * scale + f64::from(translate[1]),
            f64::from(self.width) * scale,
            f64::from(self.height) * scale,
        ]
    }
}
