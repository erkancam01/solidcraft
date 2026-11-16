//! Three-component vectors in world millimetres.

use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl V3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        V3 { x, y, z }
    }
    pub fn sonlu(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    pub fn arti(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn eksi(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn kat(self, k: f64) -> V3 {
        V3::new(self.x * k, self.y * k, self.z * k)
    }
    pub fn nokta(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn capraz(self, o: V3) -> V3 {
        V3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn boy(self) -> f64 {
        self.nokta(self).sqrt()
    }
    /// The unit vector along `self`; `None` for a zero or non-finite vector.
    pub fn birim(self) -> Option<V3> {
        let l = self.boy();
        (l.is_finite() && l > 1e-12).then(|| self.kat(1.0 / l))
    }
    /// JSON `[x, y, z]`, rounded to a nanometre so goldens stay stable.
    pub fn json(self) -> Value {
        let r = |v: f64| {
            let r = (v * 1e6).round() / 1e6;
            if r == 0.0 { 0.0 } else { r }
        };
        json!([r(self.x), r(self.y), r(self.z)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn products_are_right_handed() {
        let (x, y, z) = (V3::new(1.0, 0.0, 0.0), V3::new(0.0, 1.0, 0.0), V3::new(0.0, 0.0, 1.0));
        assert_eq!(x.capraz(y), z);
        assert_eq!(y.capraz(z), x);
        assert_eq!(z.nokta(z), 1.0);
        assert_eq!(V3::new(0.0, 0.0, 0.0).birim(), None);
        assert_eq!(V3::new(f64::NAN, 0.0, 0.0).birim(), None);
        assert_eq!(V3::new(0.0, 3.0, 4.0).boy(), 5.0);
    }
}
