use glam::Vec3;

#[derive(PartialOrd, PartialEq, Clone, Copy, Debug)]
pub enum Dist {
    Hit(f32),
    Miss,
}
use Dist::*;

impl Dist {
    pub fn unwrap_or(self, val: f32) -> f32 {
        match self {
            Hit(t) => t,
            Miss => val,
        }
    }
}

impl From<Option<f32>> for Dist {
    fn from(value: Option<f32>) -> Self {
        match value {
            Some(tee) => Self::Hit(tee),
            None => Self::Miss,
        }
    }
}

pub fn intersect_aabb(ray: Ray, bmin: Vec3, bmax: Vec3, t: f32) -> Dist {
    let tx1 = (bmin - ray.orig) / ray.dir;
    let tx2 = (bmax - ray.orig) / ray.dir;
    let tmax = tx1.max(tx2).min_element();
    let tmin = tx1.min(tx2).max_element();
    (tmax >= tmin && tmin < t && tmax > 0.)
        .then_some(tmin)
        .into()
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Ray {
    pub orig: Vec3,
    pub dir: Vec3,
}

impl Ray {
    pub fn new(orig: Vec3, dir: Vec3) -> Self {
        Self { orig, dir }
    }

    pub fn intersect(&self, [v0, v1, v2]: [Vec3; 3]) -> Dist {
        const EPS: f32 = 0.0001;
        let (edge1, edge2) = (v1 - v0, v2 - v0);
        let h = self.dir.cross(edge2);
        let a = edge1.dot(h);
        if -EPS < a && a < EPS {
            return Miss;
        }
        let f = 1. / a;
        let s = self.orig - v0;
        let u = f * s.dot(h);
        if !(0. ..=1.).contains(&u) {
            return Miss;
        }
        let q = s.cross(edge1);
        let v = f * self.dir.dot(q);
        if v < 0. || u + v > 1. {
            return Miss;
        }
        let t = f * edge2.dot(q);
        match t > EPS {
            true => Hit(t),
            false => Miss,
        }
    }
}
