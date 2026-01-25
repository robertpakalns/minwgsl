#[cfg(test)]
mod tests {
    use crate::minify;

    #[test]
    fn vertex_shader() {
        let shader = r#"
            @vertex
            fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
                return vec4<f32>(pos, 1.0);
            }
        "#;

        let min = minify(shader).unwrap();

        assert!(!min.is_empty());
        assert!(min.contains("@vertex fn"));
        assert!(min.contains("@location"));
        assert!(min.contains("@builtin(position)"));
        assert!(min.contains("vs_main"));
    }

    #[test]
    fn fragment_shader() {
        let shader = r#"
            @fragment
            fn fs_main() -> @location(0) vec4<f32> {
                let res = vec4<f32>(0.0, 1.0, 0.0, 1.0);
                return res;
            }
        "#;

        let minified = minify(shader).unwrap();

        assert!(!minified.is_empty());
        assert!(minified.contains("@fragment fn"));
        assert!(minified.contains("@location"));

        // The original variable name should be gone
        assert!(!minified.contains("res"));
    }

    #[test]
    fn remove_whitespace() {
        let shader = r#"
            fn main() -> vec4<f32> {
                return vec4<f32>(1.0, 0.0, 0.0, 1.0);
            }
        "#;

        let minified = minify(shader).unwrap();

        assert!(!minified.contains("\n"));
        assert!(!minified.contains("    "));
    }
}
