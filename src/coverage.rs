//! Cobertura to SonarQube coverage format conversion

use std::io::{BufRead, Write};
use std::path::Path;

/// Check if a file is in Cobertura XML format
pub fn is_cobertura_format(path: &Path) -> bool {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let reader = std::io::BufReader::new(file);

    for line in reader.lines().take(5).flatten() {
        let lower = line.to_lowercase();
        if lower.contains("<!doctype coverage")
            || (lower.contains("<coverage") && lower.contains("branch-rate"))
        {
            return true;
        }
        if lower.contains("<coverage version=") {
            return false;
        }
    }
    false
}

/// Convert Cobertura XML coverage to SonarQube Generic Test Data format
pub fn convert_cobertura_to_sonarqube(
    input: &Path,
    output: &Path,
    work_dir: &Path,
) -> Result<(), String> {
    use std::fs::File;
    use std::io::BufReader;

    let file =
        File::open(input).map_err(|e| format!("Failed to open coverage file: {e}"))?;
    let reader = BufReader::new(file);

    let mut out =
        File::create(output).map_err(|e| format!("Failed to create output file: {e}"))?;

    writeln!(out, r#"<?xml version="1.0"?>"#).map_err(|e| format!("Write error: {e}"))?;
    writeln!(out, r#"<coverage version="1">"#).map_err(|e| format!("Write error: {e}"))?;

    let work_dir_prefix = prepare_work_dir_prefix(work_dir);
    let work_dir_str = work_dir_prefix.trim_end_matches('/');
    let work_dir_raw_str = work_dir.to_string_lossy().to_string();
    let work_dir_raw_prefix = format!("{}/", work_dir_raw_str.trim_end_matches('/'));

    let mut source_prefix: Option<String> = None;
    let mut current_file: Option<String> = None;
    let mut lines_buffer: Vec<(u32, bool)> = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        // Handle <source> element
        if let Some(source) = process_source_element(&line) {
            if source_prefix.is_none() {
                let relative = make_source_relative_with_raw(
                    source,
                    work_dir_str,
                    &work_dir_prefix,
                    &work_dir_raw_str,
                    &work_dir_raw_prefix,
                );
                source_prefix = Some(relative);
            }
            continue;
        }

        // Handle <class> element
        if let Some(filename) = process_class_element(&line) {
            if let Some(ref file_path) = current_file {
                write_sonar_file(&mut out, file_path, &lines_buffer)?;
            }
            lines_buffer.clear();
            let full_path = combine_source_with_filename(&source_prefix, filename);
            let full_path = normalize_combined_path(
                full_path,
                work_dir_str,
                &work_dir_prefix,
                &work_dir_raw_str,
                &work_dir_raw_prefix,
            );
            current_file = Some(full_path);
            continue;
        }

        // Handle <line> element
        if let Some(coverage) = process_line_element(&line) {
            lines_buffer.push(coverage);
        }
    }

    if let Some(ref file_path) = current_file {
        write_sonar_file(&mut out, file_path, &lines_buffer)?;
    }

    writeln!(out, "</coverage>").map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

fn prepare_work_dir_prefix(work_dir: &Path) -> String {
    let work_dir_str = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf())
        .to_string_lossy()
        .to_string();
    format!("{}/", work_dir_str.trim_end_matches('/'))
}

fn extract_xml_attr(line: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr_name);
    let start = line.find(&pattern)?;
    let rest = &line[start + pattern.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_xml_attr_u32(line: &str, attr_name: &str) -> Option<u32> {
    extract_xml_attr(line, attr_name).and_then(|v| v.parse().ok())
}

fn process_class_element(line: &str) -> Option<String> {
    if !line.contains("<class") || !line.contains("filename=") {
        return None;
    }
    extract_xml_attr(line, "filename")
}

fn process_line_element(line: &str) -> Option<(u32, bool)> {
    if !line.contains("<line") || !line.contains("number=") || !line.contains("hits=") {
        return None;
    }
    let line_num = extract_xml_attr_u32(line, "number")?;
    let hits = extract_xml_attr_u32(line, "hits")?;
    Some((line_num, hits > 0))
}

fn process_source_element(line: &str) -> Option<String> {
    if !line.contains("<source>") || !line.contains("</source>") {
        return None;
    }
    let start = line.find("<source>")? + "<source>".len();
    let end = line.find("</source>")?;
    let source = line[start..end].trim().to_string();
    if source.is_empty() {
        None
    } else {
        Some(source)
    }
}

fn make_source_relative(source: String, work_dir_str: &str, work_dir_prefix: &str) -> String {
    if let Some(stripped) = source.strip_prefix(work_dir_prefix) {
        stripped.to_string()
    } else if let Some(stripped) = source.strip_prefix(work_dir_str) {
        stripped.trim_start_matches('/').to_string()
    } else {
        source
    }
}

fn make_source_relative_with_raw(
    source: String,
    work_dir_str: &str,
    work_dir_prefix: &str,
    work_dir_raw_str: &str,
    work_dir_raw_prefix: &str,
) -> String {
    let source = make_source_relative(source, work_dir_str, work_dir_prefix);
    if std::path::Path::new(&source).is_absolute() {
        if let Some(stripped) = source.strip_prefix(work_dir_raw_prefix) {
            stripped.to_string()
        } else if let Some(stripped) = source.strip_prefix(work_dir_raw_str) {
            stripped.trim_start_matches('/').to_string()
        } else {
            source
        }
    } else {
        source
    }
}

fn combine_source_with_filename(source_prefix: &Option<String>, filename: String) -> String {
    match source_prefix {
        Some(prefix) => {
            let prefix = prefix.trim_end_matches('/').trim_end_matches('\\');
            if prefix.is_empty() {
                filename
            } else {
                format!("{}/{}", prefix, filename)
            }
        }
        None => filename,
    }
}

fn normalize_combined_path(
    full_path: String,
    work_dir_str: &str,
    work_dir_prefix: &str,
    work_dir_raw_str: &str,
    work_dir_raw_prefix: &str,
) -> String {
    let stripped = if let Some(s) = full_path.strip_prefix(work_dir_prefix) {
        s.to_string()
    } else if let Some(s) = full_path.strip_prefix(work_dir_str) {
        s.trim_start_matches('/').to_string()
    } else {
        full_path
    };

    if std::path::Path::new(&stripped).is_absolute() {
        if let Some(s) = stripped.strip_prefix(work_dir_raw_prefix) {
            s.to_string()
        } else if let Some(s) = stripped.strip_prefix(work_dir_raw_str) {
            s.trim_start_matches('/').to_string()
        } else {
            stripped
        }
    } else {
        stripped
    }
}

fn write_sonar_file(
    out: &mut std::fs::File,
    file_path: &str,
    lines: &[(u32, bool)],
) -> Result<(), String> {
    if lines.is_empty() {
        return Ok(());
    }

    let mut deduped: std::collections::BTreeMap<u32, bool> = std::collections::BTreeMap::new();
    for &(line_num, covered) in lines {
        let entry = deduped.entry(line_num).or_insert(false);
        *entry |= covered;
    }

    writeln!(out, r#"  <file path="{}">"#, file_path)
        .map_err(|e| format!("Write error: {e}"))?;

    for (line_num, covered) in &deduped {
        writeln!(
            out,
            r#"    <lineToCover lineNumber="{}" covered="{}"/>"#,
            line_num,
            if *covered { "true" } else { "false" }
        )
        .map_err(|e| format!("Write error: {e}"))?;
    }

    writeln!(out, "  </file>").map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_attr() {
        let line = r#"<class name="Foo" filename="src/foo.rs">"#;
        assert_eq!(
            extract_xml_attr(line, "filename"),
            Some("src/foo.rs".to_string())
        );
        assert_eq!(extract_xml_attr(line, "name"), Some("Foo".to_string()));
        assert_eq!(extract_xml_attr(line, "missing"), None);
    }

    #[test]
    fn test_process_line_element() {
        let line = r#"<line number="42" hits="1" branch="false"/>"#;
        assert_eq!(process_line_element(line), Some((42, true)));

        let line = r#"<line number="10" hits="0"/>"#;
        assert_eq!(process_line_element(line), Some((10, false)));
    }

    #[test]
    fn test_process_source_element() {
        assert_eq!(
            process_source_element("  <source>/home/user/project</source>"),
            Some("/home/user/project".to_string())
        );
        assert_eq!(process_source_element("  <source></source>"), None);
        assert_eq!(process_source_element("not a source"), None);
    }

    #[test]
    fn test_combine_source_with_filename() {
        assert_eq!(
            combine_source_with_filename(&Some("src".to_string()), "main.rs".to_string()),
            "src/main.rs"
        );
        assert_eq!(
            combine_source_with_filename(&None, "main.rs".to_string()),
            "main.rs"
        );
        assert_eq!(
            combine_source_with_filename(&Some("".to_string()), "main.rs".to_string()),
            "main.rs"
        );
    }
}
