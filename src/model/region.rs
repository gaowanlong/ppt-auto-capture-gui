/// Represents a rectangular region on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_region() {
        let r = Region::new(10, 20, 100, 200);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 200);
    }

    #[test]
    fn test_is_valid() {
        assert!(Region::new(0, 0, 1, 1).is_valid());
        assert!(Region::new(100, 200, 1920, 1080).is_valid());
        assert!(!Region::new(0, 0, 0, 0).is_valid());
        assert!(!Region::new(0, 0, 100, 0).is_valid());
        assert!(!Region::new(0, 0, 0, 100).is_valid());
    }

    #[test]
    fn test_area() {
        assert_eq!(Region::new(0, 0, 10, 20).area(), 200);
        assert_eq!(Region::new(0, 0, 1920, 1080).area(), 2073600);
    }

    #[test]
    fn test_contains() {
        let r = Region::new(10, 20, 100, 200);
        assert!(r.contains(10, 20));
        assert!(r.contains(109, 219));
        assert!(r.contains(50, 100));
        assert!(!r.contains(0, 0));
        assert!(!r.contains(110, 20));
        assert!(!r.contains(10, 220));
    }

    #[test]
    fn test_zero_area_region() {
        let r = Region::new(0, 0, 0, 0);
        assert!(!r.is_valid());
        assert_eq!(r.area(), 0);
    }

    #[test]
    fn test_negative_coordinates() {
        let r = Region::new(-100, -50, 200, 100);
        assert!(r.is_valid());
        assert!(r.contains(-50, 0));
        assert!(!r.contains(-101, 0));
    }
}
