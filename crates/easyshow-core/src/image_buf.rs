/// A decoded image with tightly packed, non-premultiplied 8-bit RGBA pixels.
#[derive(Debug, Clone)]
pub struct ImageBuf {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ImageBuf {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        debug_assert_eq!(rgba.len(), width as usize * height as usize * 4, "rgba length mismatch");
        Self { width, height, rgba }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    /// Rotate by `turns` clockwise quarter turns (0..=3).
    pub fn rotated(self, turns: u8) -> Self {
        match turns % 4 {
            0 => self,
            1 => self.rotate90_cw(),
            2 => self.rotate180(),
            _ => self.rotate90_ccw(),
        }
    }

    /// Rotate 90° clockwise: (x, y) -> (h - 1 - y, x).
    pub fn rotate90_cw(self) -> Self {
        let (w, h) = (self.width as usize, self.height as usize);
        let (nw, nh) = (h, w);
        let mut out = vec![0u8; self.rgba.len()];
        for y in 0..h {
            for x in 0..w {
                let si = (y * w + x) * 4;
                let di = (x * nw + (h - 1 - y)) * 4;
                out[di..di + 4].copy_from_slice(&self.rgba[si..si + 4]);
            }
        }
        ImageBuf::new(nw as u32, nh as u32, out)
    }

    /// Rotate 90° counter-clockwise: (x, y) -> (y, w - 1 - x).
    pub fn rotate90_ccw(self) -> Self {
        let (w, h) = (self.width as usize, self.height as usize);
        let (nw, nh) = (h, w);
        let mut out = vec![0u8; self.rgba.len()];
        for y in 0..h {
            for x in 0..w {
                let si = (y * w + x) * 4;
                let di = ((w - 1 - x) * nw + y) * 4;
                out[di..di + 4].copy_from_slice(&self.rgba[si..si + 4]);
            }
        }
        ImageBuf::new(nw as u32, nh as u32, out)
    }

    pub fn rotate180(self) -> Self {
        let n = self.width as usize * self.height as usize;
        let mut out = vec![0u8; self.rgba.len()];
        for i in 0..n {
            let di = (n - 1 - i) * 4;
            out[di..di + 4].copy_from_slice(&self.rgba[i * 4..i * 4 + 4]);
        }
        ImageBuf::new(self.width, self.height, out)
    }

    pub fn flip_horizontal(self) -> Self {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0u8; self.rgba.len()];
        for y in 0..h {
            for x in 0..w {
                let si = (y * w + x) * 4;
                let di = (y * w + (w - 1 - x)) * 4;
                out[di..di + 4].copy_from_slice(&self.rgba[si..si + 4]);
            }
        }
        ImageBuf::new(self.width, self.height, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2×3 image whose red channel stores a unique id: id = y * 2 + x.
    fn labeled() -> ImageBuf {
        let mut rgba = Vec::new();
        for id in 0u8..6 {
            rgba.extend_from_slice(&[id, 0, 0, 255]);
        }
        ImageBuf::new(2, 3, rgba)
    }

    fn ids(buf: &ImageBuf) -> Vec<u8> {
        (0..buf.width * buf.height).map(|i| buf.rgba[i as usize * 4]).collect()
    }

    #[test]
    fn rotate90_cw_matches_mapping() {
        let out = labeled().rotate90_cw();
        assert_eq!((out.width, out.height), (3, 2));
        // src(x, y) -> dst(h-1-y, x): rows are [4,2,0], [5,3,1]
        assert_eq!(ids(&out), vec![4, 2, 0, 5, 3, 1]);
    }

    #[test]
    fn rotate90_ccw_matches_mapping() {
        let out = labeled().rotate90_ccw();
        assert_eq!((out.width, out.height), (3, 2));
        // rows are [1,3,5], [0,2,4]
        assert_eq!(ids(&out), vec![1, 3, 5, 0, 2, 4]);
    }

    #[test]
    fn rotate180_and_flip() {
        assert_eq!(ids(&labeled().rotate180()), vec![5, 4, 3, 2, 1, 0]);
        assert_eq!(ids(&labeled().flip_horizontal()), vec![1, 0, 3, 2, 5, 4]);
    }
}
