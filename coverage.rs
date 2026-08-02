pub struct CoverageManager {
    lines_covered: usize,
    total_lines: usize,
}

impl CoverageManager {
    pub fn new(total_lines: usize) -> Self {
        Self {
            lines_covered: 0,
            total_lines,
        }
    }

    pub fn mark_covered(&mut self, line: usize) -> bool {
        if line < self.total_lines {
            self.lines_covered = std::cmp::min(self.total_lines, self.lines_covered + 1);
            true
        } else {
            false
        }
    }

    pub fn get_percentage(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        (self.lines_covered as f64 / self.total_lines as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_calculation() {
        let mut cm = CoverageManager::new(10);
        assert_eq!(cm.get_percentage(), 0.0);
        cm.mark_covered(0);
        cm.mark_covered(1);
        assert_eq!(cm.get_percentage(), 20.0);
    }
}