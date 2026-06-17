use scirust_symreg::discover;
use scirust_solvers::{roots::brent, ode::rk4_fixed, quadrature::simpson_adaptive, linalg::Matrix};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscoveredLaw {
    pub expression: String,
    pub mse: f64,
    pub complexity: usize,
}

/// Symbolic regression + numerical engine backed by scirust crates.
pub struct SymbolicEngine;

impl SymbolicEngine {
    /// Discover closed-form expressions from data using genetic programming.
    pub fn discover_laws(
        inputs: &[Vec<f64>],
        outputs: &[f64],
        pop_size: usize,
        generations: usize,
    ) -> Vec<DiscoveredLaw> {
        let n = inputs.len();
        if n == 0 || inputs.first().map_or(0, |v| v.len()) == 0 {
            return Vec::new();
        }

        let data: Vec<(Vec<f64>, f64)> = inputs
            .iter()
            .zip(outputs.iter())
            .map(|(inp, out)| (inp.clone(), *out))
            .collect();

        let n_vars = inputs[0].len();
        let varnames: Vec<String> = (0..n_vars).map(|i| format!("x{}", i)).collect();
        let varname_refs: Vec<&str> = varnames.iter().map(|s| s.as_str()).collect();

        let seeds: Vec<u64> = vec![42, 123, 456];

        let pareto = discover(
            &data,
            &varname_refs,
            &seeds,
            pop_size,
            generations,
            10,
            5,
        );

        pareto.into_iter().map(|(complexity, mse, expr)| {
            DiscoveredLaw {
                expression: format!("{}", expr),
                mse,
                complexity,
            }
        }).collect()
    }

    /// Solve linear system Ax = b via LU decomposition
    pub fn solve_linear(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
        let n = a.len();
        if n == 0 || a[0].len() != n {
            return None;
        }
        let flat: Vec<f64> = a.iter().flat_map(|row| row.iter().copied()).collect();
        let mat = Matrix::from_row_major(n, n, flat);
        let inv = mat.inverse().ok()?;
        inv.matvec(b).ok()
    }

    /// Find roots using Brent's method
    pub fn find_root<F: Fn(f64) -> f64>(f: F, a: f64, b: f64) -> Option<f64> {
        let tol = scirust_solvers::Tolerance::default();
        brent(f, a, b, tol).ok().map(|sol| sol.into_inner())
    }

    /// Numerical integration with adaptive Simpson
    pub fn integrate<F: Fn(f64) -> f64>(f: F, a: f64, b: f64) -> f64 {
        simpson_adaptive(f, a, b, 1e-8, 20).unwrap_or(0.0)
    }

    /// Solve ODE with fixed-step RK4
    pub fn solve_ode<F: Fn(f64, &[f64]) -> Vec<f64>>(
        f: F,
        y0: &[f64],
        t_span: (f64, f64),
        n_steps: usize,
    ) -> Vec<Vec<f64>> {
        let h = (t_span.1 - t_span.0) / n_steps as f64;
        let wrapped = move |_t: f64, y: &[f64], dy: &mut [f64]| {
            let result = f(_t, y);
            dy.copy_from_slice(&result);
        };
        let results = rk4_fixed(wrapped, t_span.0, t_span.1, y0.to_vec(), h);
        results.into_iter().map(|(_, y)| y).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_linear_law() {
        let inputs: Vec<Vec<f64>> = (0..20).map(|i| vec![i as f64]).collect();
        let outputs: Vec<f64> = inputs.iter().map(|x| 2.0 * x[0] + 1.0).collect();
        let laws = SymbolicEngine::discover_laws(&inputs, &outputs, 100, 20);
        assert!(!laws.is_empty(), "Should discover at least one law");
    }

    #[test]
    fn test_integrate() {
        let result = SymbolicEngine::integrate(|x| x.sin(), 0.0, std::f64::consts::PI);
        assert!((result - 2.0).abs() < 0.01, "∫sin should be ~2");
    }
}
