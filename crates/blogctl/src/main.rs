//! blogctl：Research Blog 的命令行工具。构建、预览、检查和新建站点都在这里。

use anyhow::{bail, Result};
use blog_site::{article, builder, check, init, preview, profile, site, tools, zen};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[cfg(feature = "agent")]
mod agent;

#[derive(Parser)]
#[command(
    name = "blogctl",
    version,
    about = "Research Blog：构建、预览和检查研究博客"
)]
struct Cli {
    /// 博客目录（含 site/config.toml）。默认从当前目录向上查找。
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 新建一个博客目录
    Init {
        /// 新目录；必须不存在或为空
        destination: PathBuf,
        /// 网站标题
        #[arg(long)]
        name: String,
        /// 公开网站根地址，支持子路径
        #[arg(long)]
        url: String,
        /// 公开显示的作者名称
        #[arg(long, default_value = "Researcher")]
        author: String,
        /// 只检查路径并列出摘要，不写文件
        #[arg(long)]
        dry_run: bool,
    },
    /// 新建一篇草稿
    New {
        /// 文章标题，支持中文、空格和标点
        title: String,
    },
    /// 构建公开网站到 site/public/
    Build {
        /// 临时换一个网站地址
        #[arg(long)]
        base_url: Option<String>,
        /// 不重新编译 Typst 图形
        #[arg(long)]
        skip_figures: bool,
    },
    /// 构建并检查公开网站
    Check {
        /// 临时换一个网站地址
        #[arg(long)]
        base_url: Option<String>,
    },
    /// 只检查已经构建好的网站
    Validate {
        /// 含 config.toml 的网站目录，默认 site/
        #[arg(long)]
        site: Option<PathBuf>,
        /// 构建输出目录，默认 SITE/public
        #[arg(long)]
        output: Option<PathBuf>,
        /// 构建时用的网站地址，默认读 config.toml
        #[arg(long)]
        base_url: Option<String>,
    },
    /// 实时预览，含草稿
    Preview {
        /// 不自动打开浏览器
        #[arg(long)]
        no_open: bool,
    },
    /// 从 profile/cv.typ 生成个人介绍和简历
    Resume {
        /// 不生成含私人联系方式的本地简历
        #[arg(long)]
        public_only: bool,
    },
    /// 从 ZEN.md 同步样式参数
    Zen {
        /// 只检查，不写文件
        #[arg(long)]
        check: bool,
    },
    /// 检查并下载 Zola 和 Typst
    Tools {
        /// 只报告，不下载
        #[arg(long)]
        check: bool,
        /// 即使版本正确也重新下载
        #[arg(long)]
        force: bool,
        /// 在标准输出打印 JSON 摘要
        #[arg(long)]
        json: bool,
        /// 加在下载地址前面的镜像前缀，也可以用环境变量 RESEARCH_BLOG_DOWNLOAD_PREFIX
        #[arg(long)]
        proxy_prefix: Option<String>,
    },
    /// 文章助手：建索引、开本地问答服务
    #[cfg(feature = "agent")]
    #[command(subcommand)]
    Agent(agent::AgentCommand),
}

/// The nearest directory at or above the current one that holds site/config.toml.
fn find_root(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = explicit {
        if !root.join("site").join("config.toml").is_file() {
            bail!("{} 里没有 site/config.toml，不是博客目录。", root.display());
        }
        return Ok(std::path::absolute(root)?);
    }
    let current = std::env::current_dir()?;
    for directory in current.ancestors() {
        if directory.join("site").join("config.toml").is_file() {
            return Ok(directory.to_path_buf());
        }
    }
    bail!("当前目录和上级目录里都没有 site/config.toml。请进入博客目录，或者用 --root 指定。")
}

fn check_site(
    root: &Path,
    site: Option<PathBuf>,
    output: Option<PathBuf>,
    base_url: Option<&str>,
) -> Result<()> {
    let site = site.unwrap_or_else(|| root.join("site"));
    let summary = check::validate_site(&site, output.as_deref(), base_url)?;
    println!(
        "PASS: {} pages, {} local links/anchors, {} searchable article(s), feed, sitemap and static-only output",
        summary.pages, summary.links, summary.articles
    );
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init {
            destination,
            name,
            url,
            author,
            dry_run,
        } => {
            let summary = init::initialize(&destination, &name, &url, &author, dry_run)?;
            println!("{}", serde_json::to_string_pretty(&summary.to_json())?);
            if !dry_run {
                println!(
                    "下一步：cd {}，然后运行 blogctl preview。",
                    summary.destination.display()
                );
            }
        }
        Commands::Tools {
            check,
            force,
            json,
            proxy_prefix,
        } => {
            let root = find_root(cli.root).unwrap_or(std::env::current_dir()?);
            let prefix =
                proxy_prefix.or_else(|| std::env::var("RESEARCH_BLOG_DOWNLOAD_PREFIX").ok());
            if !tools::run_setup(&root, check, force, json, prefix.as_deref())? {
                std::process::exit(1);
            }
        }
        Commands::New { title } => {
            let root = find_root(cli.root)?;
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let article =
                article::create_article(&title, &root.join("site/content/posts"), &today)?;
            println!("已创建草稿：{}", article.display());
        }
        Commands::Build {
            base_url,
            skip_figures,
        } => {
            let root = find_root(cli.root)?;
            builder::build(
                &root,
                &builder::BuildOptions {
                    base_url: base_url.as_deref(),
                    skip_figures,
                },
            )?;
        }
        Commands::Check { base_url } => {
            let root = find_root(cli.root)?;
            builder::build(
                &root,
                &builder::BuildOptions {
                    base_url: base_url.as_deref(),
                    skip_figures: false,
                },
            )?;
            check_site(&root, None, None, base_url.as_deref())?;
        }
        Commands::Validate {
            site,
            output,
            base_url,
        } => {
            let root = match &site {
                Some(_) => cli.root.unwrap_or_default(),
                None => find_root(cli.root)?,
            };
            check_site(&root, site, output, base_url.as_deref())?;
        }
        Commands::Preview { no_open } => {
            let root = find_root(cli.root)?;
            preview::preview(&root, !no_open)?;
        }
        Commands::Resume { public_only } => {
            let root = find_root(cli.root)?;
            if !site::profile_enabled(&site::read_config(&root.join("site"))?)? {
                bail!("个人资料功能未启用。请先添加 profile/cv.typ，并在 site/config.toml 中设置 extra.profile_enabled = true。");
            }
            site::require_profile_source(&root)?;
            profile::build_profile(&root, !public_only)?;
            println!("个人资料源文件：{}", root.join("profile/cv.typ").display());
            if !public_only {
                println!("本地简历：{}", root.join("output/pdf/resume.pdf").display());
            }
            println!(
                "公开简历：{}",
                root.join("output/pdf/resume-public.pdf").display()
            );
            println!(
                "网站简历：{}",
                root.join("site/static/resume.pdf").display()
            );
            println!("个人介绍数据已更新；运行预览或构建网站即可查看 /about/。");
        }
        Commands::Zen { check } => {
            let root = find_root(cli.root)?;
            let changed = zen::sync(&root, check)?;
            if changed.is_empty() {
                println!("ZEN: all consumers are current.");
            } else {
                println!("ZEN: synced {} files.", changed.len());
            }
        }
        #[cfg(feature = "agent")]
        Commands::Agent(command) => {
            let root = find_root(cli.root)?;
            agent::run(&root, command)?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("错误：{error:#}");
            ExitCode::FAILURE
        }
    }
}
