use std::marker::PhantomData;

// Representation of an up to 32x32x32 brick of values
// with apron of 1 around each size
//
pub struct Brick<T>
where
    T: bytemuck::Pod,
{
    vx: usize,
    vy: usize,
    vz: usize,
    aw: usize,
    start: usize,
    end: usize,
    phantom: PhantomData<T>,
}

impl<T> Brick<T>
where
    T: bytemuck::Pod,
{
    pub fn new(vx: usize, vy: usize, vz: usize, aw: usize, start: usize, end: usize) -> Self {
        Self {
            vx,
            vy,
            vz,
            aw,
            start,
            end,
            phantom: PhantomData,
        }
    }

    /// query a single voxel at (x,y,z) in the global coordinate space
    pub fn query(&self, x: usize, y: usize, z: usize, data: &[u8]) -> T {
        // convert to local coordinates
        let x = x % self.vx;
        let y = y % self.vy;
        let z = z % self.vz;

        // fully skipped stride has pre-apron and post-apron
        let stride_x = self.vx + self.aw + self.aw;
        let stride_y = self.vy + self.aw + self.aw;

        // unskipped stride only has pre-apron
        let index = (x + self.aw) + (y + self.aw) * stride_x + (z + self.aw) * stride_x * stride_y;

        bytemuck::cast_slice::<u8, T>(&data[self.start..self.end])[index]
    }
}
