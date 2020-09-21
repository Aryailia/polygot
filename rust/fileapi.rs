use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct FileApi(PathBuf);

type Output = Result<String, String>;
impl FileApi {
    pub fn from_filename(filename: &str, directory: &str) -> Result<Self, String> {
        filename
            .rfind('.')
            .and_then(|index| {
                if filename.rfind('.').unwrap_or(0) >= index || index == 0 {
                    Some(&filename[index + '.'.len_utf8()..])
                } else {
                    None
                }
            })
            .ok_or(format!("{:?} has no file extension", filename))
            .and_then(|extension| {
                let command = Path::new(directory).join(Path::new(extension));
                if command.is_file() {
                    Ok(Self(command))
                } else {
                    Err(format!(
                        "API handler for {:?} files not found in {:?}",
                        extension, directory
                    ))
                }
            })
    }

    // These three lines are the what each file extension API must implement
    #[inline]
    pub fn comment(&self) -> Output {
        command_run(self.0.as_path(), None, &["comment"])
    }
    #[inline]
    pub fn compile(&self, stdin: &[&str], toc_location: &str, body_location: &str) -> Output {
        command_run(
            self.0.as_path(),
            Some(stdin),
            &["compile", toc_location, body_location],
        )
    }
    #[inline]
    pub fn frontmatter(&self, stdin: &[&str]) -> Output {
        command_run(self.0.as_path(), Some(stdin), &["frontmatter"])
    }
}

pub fn command_run(cmd_path: &Path, stdin: Option<&[&str]>, args: &[&str]) -> Output {
    let mut child = Command::new(cmd_path)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| format!("{:?} {}", cmd_path.display(), err))?;

    // Write the stdin if requires stdin
    if let Some(text_parts) = stdin {
        child
            .stdin
            .as_mut()
            .map(|handle| {
                for part in text_parts {
                    if let Err(err) = handle.write_all(part.as_bytes()) {
                        return Err(err.to_string());
                    }
                }
                Ok(())
            })
            .unwrap_or_else(|| Err("Cannot get handle to STDIN.".to_string()))
            .map_err(|err| {
                format!(
                    "Trouble writing to the STDIN of the {:?} command.\n{}",
                    cmd_path.display(),
                    err
                )
            })?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("Error executing {:?}. {}", cmd_path.display(), err))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|_| {
            format!(
                "{:?} returned invalid UTF8. We only support posts formatted in UTF8.",
                cmd_path.display()
            )
        })
    } else {
        Err(format!(
            "\nError code: {}\n=== STDOUT ===\n{}\n=== STDERR ===\n{}\n",
            match output.status.code() {
                Some(code) => code.to_string(),
                _ => "Interrupted".to_string(),
            },
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}
