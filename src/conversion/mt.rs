//! Mersenne Twister (MT19937) avec l'initialisation et `random()` de Python :
//! le bruit du réacteur, synthétisé par P-25 avec `random.seed(1)`, est ainsi
//! reproduit à l'identique.

pub struct Mt {
    etat: [u32; 624],
    i: usize,
}

impl Mt {
    fn init(graine: u32) -> Self {
        let mut etat = [0u32; 624];
        etat[0] = graine;
        for i in 1..624 {
            etat[i] = 1_812_433_253u32.wrapping_mul(etat[i - 1] ^ (etat[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Mt { etat, i: 624 }
    }

    /// `random.seed(n)` pour un entier `n` < 2³² : `init_by_array([n])`.
    pub fn python(n: u32) -> Self {
        let mut m = Mt::init(19_650_218);
        let cle = [n];
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..624usize.max(cle.len()) {
            let p = m.etat[i - 1];
            m.etat[i] =
                (m.etat[i] ^ (p ^ (p >> 30)).wrapping_mul(1_664_525)).wrapping_add(cle[j]).wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                m.etat[0] = m.etat[623];
                i = 1;
            }
            if j >= cle.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            let p = m.etat[i - 1];
            m.etat[i] = (m.etat[i] ^ (p ^ (p >> 30)).wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                m.etat[0] = m.etat[623];
                i = 1;
            }
        }
        m.etat[0] = 0x8000_0000;
        m
    }

    fn suivant(&mut self) -> u32 {
        if self.i >= 624 {
            for k in 0..624 {
                let y = (self.etat[k] & 0x8000_0000) | (self.etat[(k + 1) % 624] & 0x7fff_ffff);
                let mut v = self.etat[(k + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908_b0df;
                }
                self.etat[k] = v;
            }
            self.i = 0;
        }
        let mut y = self.etat[self.i];
        self.i += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `random.random()` : 53 bits.
    pub fn random(&mut self) -> f64 {
        let a = f64::from(self.suivant() >> 5);
        let b = f64::from(self.suivant() >> 6);
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `random.uniform(a, b)`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comme_python() {
        // python3 -c "import random; random.seed(1); print([random.random() for _ in range(3)])"
        let mut m = Mt::python(1);
        assert_eq!(m.random(), 0.13436424411240122);
        assert_eq!(m.random(), 0.8474337369372327);
        assert_eq!(m.random(), 0.763774618976614);
    }
}
