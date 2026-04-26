#[cfg(windows)]
mod doctor_health_report {
    use std::fs;
    use std::hint::black_box;
    use std::path::{Path, PathBuf};

    use criterion::Criterion;
    use tempfile::TempDir;
    use winbrew_app::core::hash::Hasher;
    use winbrew_app::database::{self, JournalEntry, JournalWriter};
    use winbrew_app::{AppContext, doctor};
    use winbrew_models::domains::install::{InstallScope, InstallerType};
    use winbrew_models::domains::inventory::{
        MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
    };
    use winbrew_models::domains::shared::{DeploymentKind, HashAlgorithm};
    use winbrew_testing::{InstalledPackageBuilder, init_database, test_root};

    struct DoctorBenchFixture {
        _root: TempDir,
        ctx: AppContext,
    }

    impl DoctorBenchFixture {
        fn new() -> Self {
            let root = test_root();
            let config = init_database(root.path()).expect("database should initialize");
            fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

            let ctx = AppContext::from_config(&config).expect("context should build");

            Self { _root: root, ctx }
        }

        fn activate(&self) {
            database::init(&self.ctx.paths).expect("activate benchmark root");
        }

        fn package_dir(&self, package_name: &str) -> PathBuf {
            self.ctx.paths.packages.join(package_name)
        }

        fn seed_portable_package(&self, package_name: &str, version: &str) {
            let install_dir = self.package_dir(package_name);
            fs::create_dir_all(&install_dir).expect("install dir should exist");
            fs::write(install_dir.join("tool.exe"), b"portable-payload")
                .expect("portable payload should be written");

            let package = InstalledPackageBuilder::new(package_name)
                .version(version)
                .kind(InstallerType::Portable)
                .build(&install_dir);

            let conn = database::get_conn().expect("database connection should open");
            database::insert_package(&conn, &package).expect("package should insert");

            self.write_committed_journal(
                package_name,
                version,
                "portable",
                InstallerType::Portable.deployment_kind(),
                &install_dir,
            );
        }

        fn seed_msi_package(&self, package_name: &str, version: &str) {
            let install_dir = self.package_dir(package_name);
            let file_path = install_dir.join("bin").join("tool.exe");
            fs::create_dir_all(file_path.parent().expect("file parent should exist"))
                .expect("msi install dir should exist");
            fs::write(&file_path, b"msi-payload").expect("msi payload should be written");

            let package = InstalledPackageBuilder::new(package_name)
                .version(version)
                .kind(InstallerType::Msi)
                .build(&install_dir);

            let conn = database::get_conn().expect("database connection should open");
            database::insert_package(&conn, &package).expect("package should insert");

            let snapshot = MsiInventorySnapshot {
                receipt: MsiInventoryReceipt {
                    package_name: package_name.to_string(),
                    product_code: product_code_for(package_name),
                    upgrade_code: None,
                    scope: InstallScope::Installed,
                },
                files: vec![MsiFileRecord {
                    package_name: package_name.to_string(),
                    path: file_path.to_string_lossy().into_owned(),
                    normalized_path: normalize_path(&file_path),
                    hash_algorithm: Some(HashAlgorithm::Sha256),
                    hash_hex: Some(sha256_hex(b"msi-payload")),
                    is_config_file: false,
                }],
                registry_entries: Vec::new(),
                shortcuts: Vec::new(),
                components: Vec::new(),
            };

            let mut conn = database::get_conn().expect("database connection should open");
            database::replace_snapshot(&mut conn, &snapshot).expect("msi snapshot should replace");

            self.write_committed_journal(
                package_name,
                version,
                "msi",
                InstallerType::Msi.deployment_kind(),
                &install_dir,
            );
        }

        fn seed_missing_install_package(&self, package_name: &str, version: &str) {
            let install_dir = self.package_dir(package_name);
            let package = InstalledPackageBuilder::new(package_name)
                .version(version)
                .kind(InstallerType::Portable)
                .build(&install_dir);

            let conn = database::get_conn().expect("database connection should open");
            database::insert_package(&conn, &package).expect("package should insert");
        }

        fn seed_orphan_directory(&self, package_name: &str) {
            fs::create_dir_all(self.package_dir(package_name)).expect("orphan dir should exist");
        }

        fn write_committed_journal(
            &self,
            package_name: &str,
            version: &str,
            engine: &str,
            deployment_kind: DeploymentKind,
            install_dir: &Path,
        ) {
            let mut writer = JournalWriter::open_for_package(
                self.ctx.paths.root.as_path(),
                package_name,
                version,
            )
            .expect("open committed journal");
            writer
                .append(&JournalEntry::Metadata {
                    package_id: package_name.to_string(),
                    version: version.to_string(),
                    engine: engine.to_string(),
                    deployment_kind,
                    install_dir: install_dir.to_string_lossy().into_owned(),
                    dependencies: Vec::new(),
                    commands: None,
                    bin: None,
                    command_resolution: None,
                    engine_metadata: None,
                })
                .expect("write journal metadata");
            writer
                .append(&JournalEntry::Commit {
                    installed_at: "2026-04-12T00:00:00Z".to_string(),
                })
                .expect("write journal commit");
            writer.flush().expect("flush journal");
        }

        fn write_commit_only_journal(&self, package_name: &str, version: &str) {
            let mut writer = JournalWriter::open_for_package(
                self.ctx.paths.root.as_path(),
                package_name,
                version,
            )
            .expect("open commit-only journal");
            writer
                .append(&JournalEntry::Commit {
                    installed_at: "2026-04-12T00:00:00Z".to_string(),
                })
                .expect("write journal commit");
            writer.flush().expect("flush journal");
        }

        fn healthy_empty() -> Self {
            Self::new()
        }

        fn healthy_mixed() -> Self {
            let fixture = Self::new();

            for index in 0..6 {
                let package_name = format!("Contoso.Portable{index}");
                fixture.seed_portable_package(&package_name, "1.0.0");
            }

            for index in 0..2 {
                let package_name = format!("Contoso.Msi{index}");
                fixture.seed_msi_package(&package_name, "1.0.0");
            }

            fixture
        }

        fn dirty_mixed() -> Self {
            let fixture = Self::healthy_mixed();

            fixture.seed_missing_install_package("Contoso.MissingInstall", "1.0.0");
            fixture.seed_orphan_directory("Contoso.Orphan");

            let stale_install_dir = fixture.package_dir("Contoso.StaleJournal");
            fs::create_dir_all(&stale_install_dir).expect("stale install dir should exist");
            let stale_package = InstalledPackageBuilder::new("Contoso.StaleJournal")
                .version("2.0.0")
                .kind(InstallerType::Portable)
                .build(&stale_install_dir);
            let conn = database::get_conn().expect("database connection should open");
            database::insert_package(&conn, &stale_package).expect("stale package should insert");
            fixture.write_committed_journal(
                "Contoso.StaleJournal",
                "1.0.0",
                "portable",
                InstallerType::Portable.deployment_kind(),
                &stale_install_dir,
            );

            fixture.write_commit_only_journal("Contoso.LegacyJournal", "1.0.0");

            fixture
        }
    }

    fn normalize_path(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Hasher::new(HashAlgorithm::Sha256);
        hasher.update(bytes);

        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn product_code_for(package_name: &str) -> String {
        let digest = sha256_hex(package_name.as_bytes());
        format!(
            "{{{}-{}-{}-{}-{}}}",
            &digest[0..8],
            &digest[8..12],
            &digest[12..16],
            &digest[16..20],
            &digest[20..32]
        )
    }

    fn bench_report<M>(
        group: &mut criterion::BenchmarkGroup<'_, M>,
        name: &str,
        fixture: &DoctorBenchFixture,
    ) where
        M: criterion::measurement::Measurement,
    {
        fixture.activate();

        group.bench_function(name, |b| {
            b.iter(|| {
                let report = doctor::health_report(black_box(&fixture.ctx)).expect("health report");
                black_box(report);
            });
        });
    }

    pub fn bench_doctor_health_report(c: &mut Criterion) {
        let empty = DoctorBenchFixture::healthy_empty();
        let healthy = DoctorBenchFixture::healthy_mixed();
        let dirty = DoctorBenchFixture::dirty_mixed();

        let mut group = c.benchmark_group("doctor_health_report");
        bench_report(&mut group, "empty", &empty);
        bench_report(&mut group, "healthy_mixed", &healthy);
        bench_report(&mut group, "dirty_mixed", &dirty);
        group.finish();
    }
}

#[cfg(windows)]
use criterion::{criterion_group, criterion_main};

#[cfg(windows)]
use doctor_health_report::bench_doctor_health_report;

#[cfg(windows)]
criterion_group!(benches, bench_doctor_health_report);

#[cfg(windows)]
criterion_main!(benches);

#[cfg(not(windows))]
fn main() {}
