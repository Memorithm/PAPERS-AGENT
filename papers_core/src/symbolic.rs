#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredLaw {
    pub expression: String,
    pub mse: f64,
    pub complexity: usize,
}

/// Deterministic numerical baseline for PAPERS core.
///
/// Advanced symbolic regression belongs in the SciRust/CCOS runtime adapter;
/// PAPERS core keeps a small auditable affine fit and numerical toolbox so its
/// default build no longer depends on an absolute `/tmp/scirust` checkout.
pub struct SymbolicEngine;

impl SymbolicEngine {
    /// Fit an affine law `y = b + sum(ai*xi)` by least squares.
    /// This is deliberately modest: it produces measured MSE, not a fabricated
    /// symbolic-search fitness. Nonlinear discovery is delegated to SciRust.
    pub fn discover_laws(
        inputs: &[Vec<f64>],
        outputs: &[f64],
        _pop_size: usize,
        _generations: usize,
    ) -> Vec<DiscoveredLaw> {
        if inputs.is_empty() || outputs.len() != inputs.len() {
            return Vec::new();
        }
        let vars = inputs[0].len();
        if vars == 0 || inputs.iter().any(|row| row.len() != vars) {
            return Vec::new();
        }

        let p = vars + 1;
        let mut ata = vec![vec![0.0; p]; p];
        let mut aty = vec![0.0; p];
        for (row, &y) in inputs.iter().zip(outputs) {
            let mut x = Vec::with_capacity(p);
            x.push(1.0);
            x.extend(row.iter().copied());
            for i in 0..p {
                aty[i] += x[i] * y;
                for j in 0..p {
                    ata[i][j] += x[i] * x[j];
                }
            }
        }

        // Small ridge term makes singular/near-singular paper data fail softly.
        for (i, row) in ata.iter_mut().enumerate() {
            row[i] += 1e-12;
        }
        let Some(coeffs) = Self::solve_linear(&ata, &aty) else {
            return Vec::new();
        };

        let mse = inputs
            .iter()
            .zip(outputs)
            .map(|(row, &y)| {
                let predicted = coeffs[0]
                    + row
                        .iter()
                        .zip(coeffs.iter().skip(1))
                        .map(|(x, a)| x * a)
                        .sum::<f64>();
                (predicted - y).powi(2)
            })
            .sum::<f64>()
            / inputs.len() as f64;

        let mut terms = vec![format!("{:.12}", coeffs[0])];
        for (index, coefficient) in coeffs.iter().skip(1).enumerate() {
            terms.push(format!("{:+.12}*x{index}", coefficient));
        }
        vec![DiscoveredLaw {
            expression: terms.join(" "),
            mse,
            complexity: p,
        }]
    }

    /// Solve Ax=b by Gaussian elimination with partial pivoting.
    pub fn solve_linear(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
        let n = a.len();
        if n == 0 || b.len() != n || a.iter().any(|row| row.len() != n) {
            return None;
        }
        let mut m: Vec<Vec<f64>> = a
            .iter()
            .zip(b)
            .map(|(row, &rhs)| {
                let mut r = row.clone();
                r.push(rhs);
                r
            })
            .collect();

        for col in 0..n {
            let pivot = (col..n).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
            if m[pivot][col].abs() <= 1e-15 {
                return None;
            }
            m.swap(col, pivot);
            let divisor = m[col][col];
            for j in col..=n {
                m[col][j] /= divisor;
            }
            for i in 0..n {
                if i == col {
                    continue;
                }
                let factor = m[i][col];
                for j in col..=n {
                    m[i][j] -= factor * m[col][j];
                }
            }
        }
        Some((0..n).map(|i| m[i][n]).collect())
    }

    /// Deterministic bracketed root finder (bisection).
    pub fn find_root<F: Fn(f64) -> f64>(f: F, mut a: f64, mut b: f64) -> Option<f64> {
        let mut fa = f(a);
        let fb = f(b);
        if !fa.is_finite() || !fb.is_finite() || fa * fb > 0.0 {
            return None;
        }
        for _ in 0..128 {
            let mid = 0.5 * (a + b);
            let fm = f(mid);
            if !fm.is_finite() {
                return None;
            }
            if fm.abs() < 1e-12 || (b - a).abs() < 1e-12 {
                return Some(mid);
            }
            if fa * fm <= 0.0 {
                b = mid;
            } else {
                a = mid;
                fa = fm;
            }
        }
        Some(0.5 * (a + b))
    }

    /// Adaptive Simpson integration.
    pub fn integrate<F: Fn(f64) -> f64>(f: F, a: f64, b: f64) -> f64 {
        fn simpson<F: Fn(f64) -> f64>(f: &F, a: f64, b: f64) -> f64 {
            let c = (a + b) * 0.5;
            (b - a) * (f(a) + 4.0 * f(c) + f(b)) / 6.0
        }
        fn recurse<F: Fn(f64) -> f64>(
            f: &F,
            a: f64,
            b: f64,
            whole: f64,
            eps: f64,
            depth: u32,
        ) -> f64 {
            let c = (a + b) * 0.5;
            let left = simpson(f, a, c);
            let right = simpson(f, c, b);
            let delta = left + right - whole;
            if depth == 0 || delta.abs() <= 15.0 * eps {
                return left + right + delta / 15.0;
            }
            recurse(f, a, c, left, eps * 0.5, depth - 1)
                + recurse(f, c, b, right, eps * 0.5, depth - 1)
        }
        let whole = simpson(&f, a, b);
        recurse(&f, a, b, whole, 1e-8, 20)
    }

    /// Fixed-step classical RK4.
    pub fn solve_ode<F: Fn(f64, &[f64]) -> Vec<f64>>(
        f: F,
        y0: &[f64],
        t_span: (f64, f64),
        n_steps: usize,
    ) -> Vec<Vec<f64>> {
        if n_steps == 0 || y0.is_empty() {
            return Vec::new();
        }
        let h = (t_span.1 - t_span.0) / n_steps as f64;
        let mut t = t_span.0;
        let mut y = y0.to_vec();
        let mut out = Vec::with_capacity(n_steps + 1);
        out.push(y.clone());
        for _ in 0..n_steps {
            let k1 = f(t, &y);
            if k1.len() != y.len() {
                return Vec::new();
            }
            let y2: Vec<f64> = y.iter().zip(&k1).map(|(v, k)| v + 0.5 * h * k).collect();
            let k2 = f(t + 0.5 * h, &y2);
            let y3: Vec<f64> = y.iter().zip(&k2).map(|(v, k)| v + 0.5 * h * k).collect();
            let k3 = f(t + 0.5 * h, &y3);
            let y4: Vec<f64> = y.iter().zip(&k3).map(|(v, k)| v + h * k).collect();
            let k4 = f(t + h, &y4);
            if k2.len() != y.len() || k3.len() != y.len() || k4.len() != y.len() {
                return Vec::new();
            }
            for i in 0..y.len() {
                y[i] += h * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]) / 6.0;
            }
            t += h;
            out.push(y.clone());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_affine_law_with_measured_mse() {
        let inputs: Vec<Vec<f64>> = (0..20).map(|i| vec![i as f64]).collect();
        let outputs: Vec<f64> = inputs.iter().map(|x| 2.0 * x[0] + 1.0).collect();
        let laws = SymbolicEngine::discover_laws(&inputs, &outputs, 100, 20);
        assert_eq!(laws.len(), 1);
        assert!(laws[0].mse < 1e-12);
    }

    #[test]
    fn integrates_sine() {
        let result = SymbolicEngine::integrate(|x| x.sin(), 0.0, std::f64::consts::PI);
        assert!((result - 2.0).abs() < 1e-6);
    }

    #[test]
    fn solves_linear_system() {
        let x =
            SymbolicEngine::solve_linear(&[vec![2.0, 1.0], vec![1.0, -1.0]], &[5.0, 1.0]).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-9);
        assert!((x[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn finds_bracketed_root() {
        let root = SymbolicEngine::find_root(|x| x * x - 2.0, 0.0, 2.0).unwrap();
        assert!((root - 2.0_f64.sqrt()).abs() < 1e-9);
    }
}
