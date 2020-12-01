use crate::traits::ShellEscape;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct FileApi<'a> {
    pathbuf: PathBuf,
    env: Env<'a>,
}

type Env<'a> = (&'a str, &'a str);
type Output = Result<String, String>;

impl<'a> FileApi<'a> {
    pub fn from_filename(api_dir: &str, extension: &str, env: Env<'a>) -> Result<Self, String> {
        let command = Path::new(api_dir).join(Path::new(extension));
        if command.is_file() {
            Ok(Self {
                pathbuf: command,
                env,
            })
        } else {
            Err([
                "Cannot find API handler for ",
                extension.escape().as_str(),
                " not found.\nCannot read the file ",
                command.to_string_lossy().escape().as_str(),
            ]
            .join(""))
        }
    }

    // These three lines are the what each file extension API must implement
    #[inline]
    pub fn comment(&self) -> Output {
        command_run(self.pathbuf.as_path(), self.env, None, &["comment"])
    }
    #[inline]
    pub fn compile(&self, stdin: &[&str], toc_location: &str, body_location: &str) -> Output {
        command_run(
            self.pathbuf.as_path(),
            self.env,
            Some(stdin),
            &["compile", toc_location, body_location],
        )
    }
    #[inline]
    pub fn frontmatter(&self, stdin: &[&str]) -> Output {
        command_run(self.pathbuf.as_path(), self.env, Some(stdin), &["frontmatter"])
    }
}

pub fn command_run(cmd_path: &Path, env: Env, stdin: Option<&[&str]>, args: &[&str]) -> Output {
    let mut child = Command::new(cmd_path)
        .env("DOMAIN", env.0)
        .env("BLOG_RELATIVE", env.1)
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| {
            [
                "Error starting the executable ",
                cmd_path.to_string_lossy().escape().as_str(),
                "\n",
                err.to_string().as_str(),
            ]
            .join("")
        })?;

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
                [
                    "Trouble writing to the STDIN of the ",
                    cmd_path.to_string_lossy().escape().as_str(),
                    " command.",
                    "\n",
                    err.as_str(),
                ]
                .join("")
            })?;
    }

    let output = child.wait_with_output().map_err(|err| {
        [
            "The executable ",
            cmd_path.to_string_lossy().escape().as_str(),
            " could not run.\n",
            err.to_string().as_str(),
        ]
        .join("")
    })?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|_| {
            [
                cmd_path.to_string_lossy().escape().as_str(),
                " had invalid UTF8. We only support posts encoded in UTF8.",
            ]
            .join("")
        })
    } else {
        Err([
            "Error while executing ",
            cmd_path.to_string_lossy().escape().as_str(),
            "\nError code: ",
            match output.status.code() {
                Some(code) => code.to_string(),
                _ => "Interrupted".to_string(),
            }
            .as_str(),
            "\n=== STDOUT ===\n",
            String::from_utf8_lossy(&output.stdout).to_string().as_str(),
            "\n=== STDERR ===\n",
            String::from_utf8_lossy(&output.stderr).to_string().as_str(),
        ]
        .join(""))
    }
}
