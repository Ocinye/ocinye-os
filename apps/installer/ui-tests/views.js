/* Ocinye OS Installer · as 60 vistas de referência do Design (reference/d011),
   como vistas do controlador, para validar o renderer **real** (ui/app.js) nas
   três línguas e três tamanhos. Valores fictícios: domínios .test,
   192.0.2.0/24, 2001:db8::/32, impressões e somas evidentemente falsas.
   Não é produto: não é empacotado (frontendDist = ui/). */
(function () {
  'use strict';
  const FP = 'SHA256:EXEMPLO0nlyRefer3nceFingerpr1ntNotReal0000000';
  const FP_OLD = 'SHA256:EXEMPLOold0KeyRecordedPreviouslyNotReal11111';
  const H = (c) => c.repeat(64);
  const base = () => ({
    screen: 'welcome', lang: 'pt', alert: null, field_errors: {}, busy: false, stopping: false,
    app: { version: '0.1.0', platform: 'macos · aarch64' },
    draft: { host: '192.0.2.10', port: '22', user: 'operador', auth: 'agent', key_path: null, instance_name: 'Empresa Exemplo', distributions: ['business', 'research'], endpoints: [{ host: 'os.empresa.test', distribution: null, dns: 'DNS_OK', seen: ['192.0.2.10'] }, { host: 'business.empresa.test', distribution: 'business', dns: 'DNS_OK', seen: ['192.0.2.10'] }, { host: 'research.empresa.test', distribution: 'research', dns: 'DNS_OK', seen: ['192.0.2.10'] }], tls_mode: 'provided', tls_cert: '/exemplo/empresa-test.crt.pem', tls_key: '/exemplo/empresa-test.key.pem', tls_chain: '/exemplo/empresa-test.chain.pem', admin: ['Ana Exemplo', 'ana@empresa.test', 'Ana Exemplo (Admin)', 'ana.admin@empresa.test'] },
    sudo_dialog: false, presented_key: null, pinned_key: ['ssh-ed25519', FP], mismatch: null,
    release: { id: '3f9c2a7d1e04', commit: '3f9c2a7d1e04b7c19e6a5d20f81c4e7a9b03d6f2', build: 'release', arch: 'amd64', migrations: { count: 64, latest: '0064' }, artifacts: ['source.tar.gz', 'install/ocinye', 'ocinye-bootstrap', 'images/ocinye-core-server.tar', 'images/ocinye-workspace.tar', 'images/ocinye-worker.tar', 'images/ocinye-conversion-runner.tar', 'images/ocinye-converter.tar'], files: 12, package: 'ocinye-os-3f9c2a7d1e04' },
    probe: { uid: 1000, groups: ['operador', 'sudo'], sudo_nopasswd: false, machine: 'x86_64', os: 'ubuntu 24.04', pretty: 'Ubuntu 24.04.5 LTS', hostname: 'srv-01' },
    elevation: 'SudoPassword',
    facts: { hostname: 'srv-01', os: { id: 'ubuntu', version_id: '24.04', pretty: 'Ubuntu 24.04.5 LTS' }, kernel_name: 'Linux', kernel_release: '6.8.0-45-generic', machine: 'x86_64', arch: 'amd64', privilege: 'root', systemd: true, ipv4: ['192.0.2.10'], ipv6: ['2001:db8::10'] },
    preflight: null, hardware: null, plan: null, tls_validation: null, ending: null, events: [],
    verification: { items: [] }, lifecycle: null, journal: null, credential: null, credential_acknowledged: false, receipt: null, live: {},
  });

  // ── verificação prévia
  const item = (id, status, observation, action) => ({ id, status, action: action || null, observation });
  function pre(mode) {
    const it = {
      PF_OS: item('PF_OS', 'PASS', { kind: 'os', kernel_name: 'Linux', kernel_release: '6.8.0-45-generic' }),
      PF_DISTRO: item('PF_DISTRO', 'PASS', { kind: 'distro', id: 'ubuntu', version_id: '24.04', pretty: 'Ubuntu 24.04.5 LTS', graphical: false }),
      PF_ARCH: item('PF_ARCH', 'PASS', { kind: 'arch', server: 'x86_64', release: 'amd64' }),
      PF_PRIV: item('PF_PRIV', 'PASS', { kind: 'privilege', privilege: 'root' }),
      PF_TIME: item('PF_TIME', 'PASS', { kind: 'clock', skew_seconds: 0 }),
      PF_CPU: item('PF_CPU', 'PASS', { kind: 'cpu', count: 8 }),
      PF_RAM: item('PF_RAM', 'PASS', { kind: 'ram', total_mb: 32768, swap_mb: 0 }),
      PF_DISK: item('PF_DISK', 'PASS', { kind: 'disk', path: '/srv', free_gb: 860 }),
      PF_HWREAD: item('PF_HWREAD', 'PASS', { kind: 'hardware', readable: true }),
      PF_DOCKER: item('PF_DOCKER', 'PASS', { kind: 'docker', state: 'DETECTED_SUPPORTED', observed: { cli_present: true, server_version: '27.3.1', compose_v2: true, conflicts: [], docker_repo_present: true } }),
      PF_PORTS: item('PF_PORTS', 'PASS', { kind: 'ports', busy: [] }),
      PF_PROXY: item('PF_PROXY', 'PASS', { kind: 'proxy', units: [] }),
      PF_FW: item('PF_FW', 'WARNING', { kind: 'firewall', state: 'REQUIRED_RULES_CAN_BE_APPLIED', observed: { ufw_active: true, ufw_allows: [22], ufw_denies: [], firewalld_active: false, custom_input_drop: false } }, 'WILL_APPLY'),
      PF_PKG: item('PF_PKG', 'PASS', { kind: 'packages', installable: [], missing: [] }),
      PF_REGISTRY: item('PF_REGISTRY', 'PASS', { kind: 'registry', reachable: true }),
      PF_SYSTEMD: item('PF_SYSTEMD', 'PASS', { kind: 'systemd', present: true }),
      PF_EXIST: item('PF_EXIST', 'PASS', { kind: 'existing', release: null, config_present: false }),
      PF_JOURNAL: item('PF_JOURNAL', 'PASS', { kind: 'journal', found: false }),
      PF_SRVDIR: item('PF_SRVDIR', 'PASS', { kind: 'srv_dir', non_empty: false, journalled: false }),
    };
    if (mode === 'warning') { it.PF_RAM = item('PF_RAM', 'WARNING', { kind: 'ram', total_mb: 4096, swap_mb: 0 }); it.PF_CPU = item('PF_CPU', 'WARNING', { kind: 'cpu', count: 2 }); }
    if (mode === 'blocked') it.PF_DISK = item('PF_DISK', 'BLOCKED', { kind: 'disk', path: '/srv', free_gb: 9 });
    if (mode === 'existing') { it.PF_EXIST = item('PF_EXIST', 'BLOCKED', { kind: 'existing', release: '1a2b3c4d5e6f', config_present: true }); it.PF_PORTS = item('PF_PORTS', 'BLOCKED', { kind: 'ports', busy: [{ port: 443, process: 'docker-proxy', ocinye: true }] }); }
    if (mode === 'runtime') { it.PF_DOCKER = item('PF_DOCKER', 'WARNING', { kind: 'docker', state: 'MISSING_INSTALLABLE', observed: { cli_present: false, server_version: null, compose_v2: false, conflicts: [], docker_repo_present: false } }, 'WILL_INSTALL'); }
    if (mode === 'conflict') it.PF_DOCKER = item('PF_DOCKER', 'BLOCKED', { kind: 'docker', state: 'CONFLICTING_RUNTIME', observed: { cli_present: true, server_version: null, compose_v2: false, conflicts: ['DISTRO_DOCKER_IO'], docker_repo_present: false } });
    if (mode === 'fwext') it.PF_FW = item('PF_FW', 'WARNING', { kind: 'firewall', state: 'EXTERNAL_FIREWALL_ACTION_REQUIRED', observed: { ufw_active: false, ufw_allows: [], ufw_denies: [], firewalld_active: true, custom_input_drop: false } }, 'EXTERNAL_ACTION');
    if (mode === 'port') it.PF_PORTS = item('PF_PORTS', 'BLOCKED', { kind: 'ports', busy: [{ port: 80, process: 'caddy', ocinye: false }] });
    return { items: Object.values(it), incomplete: null };
  }

  // ── hardware
  const gpu = (i, model, vendor, vram, drv, rt, cap) => ({ index: i, pci_address: '0000:0' + (i + 1) + ':00.0', vendor: { vendor }, device_id: 8752, model, vram_bytes: vram * 1e9, driver: { module: vendor === 'amd' ? 'amdgpu' : 'nvidia', version: drv }, compute_capability: cap, runtime: rt });
  function hw(kind) {
    const h = { cpu: { model: 'AMD EPYC 7443P', arch: 'amd64', cores: 24, threads: 48, features: ['avx2'] }, memory: { total_bytes: 128e9, available_bytes: 124e9, swap_bytes: 0 }, storage: { path: '/srv', free_bytes: 1.8e12, total_bytes: 1.9e12, filesystem: 'ext4' }, network: { interfaces_up: 2, max_link_mbps: 10000, ipv6: true }, gpu: { status: 'NO_GPU', basic_display: null }, security: { confidential: [], tpm: 'NOT_DETECTED' } };
    if (kind === 'cpu-only') { h.cpu.model = 'AMD EPYC 7313P'; h.cpu.cores = 16; h.cpu.threads = 32; }
    if (kind === 'no-gpu') { h.cpu.model = 'Intel Xeon E-2388G'; h.gpu = { status: 'NO_GPU', basic_display: 'ASPEED' }; }
    if (kind === 'one-gpu') { h.gpu = { status: 'GPU_DETECTED', gpus: [gpu(0, 'NVIDIA RTX 4090', 'nvidia', 24, '550.54', ['nvidia_container_toolkit', 'detected'], '8.9')] }; h.security = { confidential: [['amd_sev_snp', 'DETECTED']], tpm: 'DETECTED' }; }
    if (kind === 'multi-gpu') { h.gpu = { status: 'GPU_DETECTED', gpus: [gpu(0, 'NVIDIA RTX A6000', 'nvidia', 48, '550.54', ['nvidia_container_toolkit', 'detected'], '8.6'), gpu(1, 'NVIDIA RTX A6000', 'nvidia', 48, '550.54', ['nvidia_container_toolkit', 'detected'], '8.6'), gpu(2, 'NVIDIA RTX 4090', 'nvidia', 24, '550.54', ['nvidia_container_toolkit', 'detected'], '8.9'), gpu(3, 'AMD Radeon PRO W7800', 'amd', 32, '6.8', ['rocm', 'detected'], 'gfx1100')] }; h.security = { confidential: [['amd_sev_snp', 'DETECTED']], tpm: 'DETECTED' }; }
    if (kind === 'runtime-unavailable') h.gpu = { status: 'GPU_RUNTIME_UNAVAILABLE', gpus: [gpu(0, 'NVIDIA RTX 4090', 'nvidia', 24, '550.54', ['nvidia_container_toolkit', 'unavailable'], '8.9')] };
    if (kind === 'discovery-error') h.gpu = { status: 'GPU_DISCOVERY_ERROR', code: 'EACCES' };
    return h;
  }

  // ── plano e execução
  const plan = (tls) => ({
    plan_id: 'pl-5d0a910000000000', installation_id: 'inst-7c41e20000000000', created_at: '2026-10-04T14:16:31Z',
    release: { id: '3f9c2a7d1e04', commit: '3f9c2a7d1e04b7c19e6a5d20f81c4e7a9b03d6f2', build: 'release', arch: 'amd64', manifest_sha256: H('a') },
    target: { host: { kind: 'v4', value: '192.0.2.10' }, port: 22, user: 'operador', host_key_algorithm: 'ssh-ed25519', host_key_sha256: FP },
    configuration: { instance_name: 'Empresa Exemplo', distributions: ['business', 'research'], endpoints: { canonical: 'os.empresa.test', bound: [{ host: 'business.empresa.test', distribution: 'business' }, { host: 'research.empresa.test', distribution: 'research' }] }, tls: tls === 'self' ? { mode: 'SELF_SIGNED_TEST' } : { mode: 'OPERATOR_SUPPLIED', cert_sha256: H('c'), not_after: '2027-09-30T00:00:00Z', covers: ['os.empresa.test', '*.empresa.test'], chain: true }, admin: { person: { name: 'Ana Exemplo', email: 'ana@empresa.test' }, privileged: { name: 'Ana Exemplo (Admin)', email: 'ana.admin@empresa.test' } } },
    system_changes: [{ op: 'install_docker_from_official_repo', target: 'ubuntu-24.04', arch: 'amd64', packages: ['docker-ce', 'docker-ce-cli', 'containerd.io', 'docker-compose-plugin'], repo_key_fingerprint: '9DC858229FC7DD38854AE2D88D81803C0EBFCD88' }, { op: 'firewall_allow', manager: 'ufw', port: 80, proto: 'tcp', comment: 'ocinye' }, { op: 'firewall_allow', manager: 'ufw', port: 443, proto: 'tcp', comment: 'ocinye' }],
    preflight: [], hardware: { threads: 48, memory_mb: 131072, gpus: 1, compute_readiness: 'GPU_FUTURE_CANDIDATE', provider_mode: 'OFF' },
    phases: ['P01', 'P02', 'P03', 'P04', 'P05', 'P06', 'P07', 'P08', 'P09', 'P10', 'P11', 'P12', 'P13', 'P14', 'P15', 'P16'].map((p) => ({ phase: p, skip: false })),
    plan_sha256: '4be1' + H('0').slice(8) + '0c7e',
  });
  const ph = (n, extra) => { const o = {}; ['P01', 'P02', 'P03', 'P04', 'P05', 'P06', 'P07', 'P08', 'P09', 'P10', 'P11', 'P12', 'P13', 'P14', 'P15', 'P16'].slice(0, n).forEach((p) => { o[p] = 'done'; }); return Object.assign(o, extra || {}); };
  const live = (n, extra, more) => Object.assign({ elapsed_s: 242, phases: ph(n, extra), ops: {}, current: 'P' + String(n + 1).padStart(2, '0'), upload: null, warnings: 0, events: [], verification: [] }, more || {});
  const ver = (mode) => {
    const p = (id, ev) => ({ id, status: 'PASS', evidence: ev });
    const server = [p('V01', 'SERVICES_HEALTHY'), p('V02', 'DATABASE_REACHABLE'), p('V03', 'SCHEMA_0064_0'), p('V04', 'READY_READY'), p('V05', 'INSTANCE_MATCHES'), p('V06', 'DISTRIBUTIONS_MATCH'), p('V07', 'APPLICATIONS_20_28'), p('V08', 'STORAGE_AVAILABLE'), p('V09', 'HTTP_303_200'), p('V12b', 'ENDPOINTS_MATCH'), p('V16', 'FIRST_ACCESS_PENDING')];
    const local = [mode === 'dns' ? { id: 'V10', status: 'PENDING', evidence: 'DNS_UNRESOLVED:research.empresa.test' } : p('V10', 'DNS_OK'), p('VFw', 'PORTS_REACHABLE'), p('V11', mode === 'test' ? 'TLS_SELF_SIGNED_PINNED' : 'TLS_VALID'), p('V12', 'ENDPOINTS_ANSWER'), p('V13', 'LOGIN_200')];
    if (mode === 'fail') { server[3] = { id: 'V04', status: 'FAIL', evidence: 'READY_BLOCKED' }; return { items: server.slice(0, 4) }; }
    return { items: server.concat(local) };
  };

  const V = {};
  const set = (name, f) => { V[name] = () => { const v = base(); f(v); return v; }; };
  set('welcome', (v) => { v.release = null; v.probe = null; });
  set('release', (v) => { v.screen = 'release'; });
  set('release-dev', (v) => { v.screen = 'release'; v.release.build = 'proof'; v.release.package = 'ocinye-os-3f9c2a7d1e04-proof'; });
  set('release-checksum', (v) => { v.screen = 'release'; v.alert = { code: 'RELEASE_CHECKSUM_FAILED', params: { file: 'images/ocinye-core-server.tar' } }; });
  set('release-arch', (v) => { v.screen = 'release'; v.alert = { code: 'RELEASE_UNSUPPORTED_ARCH', params: { arch: 'riscv64' } }; });
  set('release-manifest', (v) => { v.screen = 'release'; v.alert = { code: 'INVALID_MANIFEST', params: { field: 'migrations.latest' } }; });
  set('release-target', (v) => { v.screen = 'server'; v.alert = { code: 'RELEASE_UNSUPPORTED_TARGET', params: { release: 'amd64', server: 'aarch64' } }; v.probe = null; });
  set('server', (v) => { v.screen = 'server'; });
  set('server-key', (v) => { v.screen = 'server'; v.draft.auth = 'key'; v.draft.key_path = '/Users/exemplo/.ssh/id_ed25519'; v.probe = null; });
  set('server-invalid', (v) => { v.screen = 'server'; v.draft.host = 'srv-01.empresa.test; rm'; v.field_errors = { host: 'INPUT_INVALID_HOST' }; v.probe = null; });
  set('server-unreachable', (v) => { v.screen = 'server'; v.alert = { code: 'SSH_UNREACHABLE', params: {} }; v.probe = null; });
  set('server-auth', (v) => { v.screen = 'server'; v.alert = { code: 'SSH_AUTH_REFUSED', params: {} }; v.probe = null; });
  set('host-trust', (v) => { v.screen = 'host-trust'; v.presented_key = ['ssh-ed25519', FP]; v.probe = null; });
  set('host-mismatch', (v) => { v.screen = 'host-mismatch'; v.mismatch = [FP_OLD, FP]; v.probe = null; });
  set('privilege-sudo', (v) => { v.screen = 'preflight'; v.sudo_dialog = true; });
  set('privilege-missing', (v) => { v.screen = 'preflight'; v.alert = { code: 'PRIVILEGE_MISSING', params: {} }; });
  set('preflight-running', (v) => { v.screen = 'preflight'; v.busy = true; });
  set('preflight-pass', (v) => { v.screen = 'preflight'; v.preflight = pre('pass'); });
  set('preflight-warning', (v) => { v.screen = 'preflight'; v.preflight = pre('warning'); });
  set('preflight-blocked', (v) => { v.screen = 'preflight'; v.preflight = pre('blocked'); });
  set('preflight-port', (v) => { v.screen = 'preflight'; v.preflight = pre('port'); });
  set('preflight-runtime-conflict', (v) => { v.screen = 'preflight'; v.preflight = pre('conflict'); });
  set('preflight-runtime-missing', (v) => { v.screen = 'preflight'; v.preflight = pre('runtime'); });
  set('preflight-firewall-external', (v) => { v.screen = 'preflight'; v.preflight = pre('fwext'); });
  set('preflight-existing', (v) => { v.screen = 'preflight'; v.preflight = pre('existing'); });
  set('incomplete', (v) => { v.screen = 'incomplete'; v.journal = { installation_id: 'inst-7c41e20000000000', plan_id: 'pl-5d0a910000000000', release: '3f9c2a7d1e04', state: 'RUNNING', completed: ['P01', 'P02', 'P03', 'P04', 'P05', 'P06', 'P07', 'P08'], current: 'P09', failed_code: null, instance_created: false, self_signed_cert_sha256: null, updated_at: '2026-10-04T14:22:51Z' }; });
  ['cpu-only', 'no-gpu', 'one-gpu', 'multi-gpu', 'runtime-unavailable', 'discovery-error'].forEach((k) => set('hardware-' + k, (v) => { v.screen = 'hardware'; v.hardware = hw(k); }));
  set('compute-readiness-future', (v) => { v.screen = 'hardware'; v.hardware = hw('one-gpu'); });
  set('instance', (v) => { v.screen = 'instance'; });
  set('distributions', (v) => { v.screen = 'distributions'; });
  set('distributions-none', (v) => { v.screen = 'distributions'; v.draft.distributions = []; v.field_errors = { dist: 'NO_DISTRIBUTION' }; });
  set('endpoints', (v) => { v.screen = 'endpoints'; });
  set('endpoints-dns-pending', (v) => { v.screen = 'endpoints'; v.draft.endpoints[1].dns = 'DNS_UNRESOLVED'; v.draft.endpoints[1].seen = []; v.draft.endpoints[2].dns = 'DNS_REQUIRED'; });
  set('endpoints-dns-wrong', (v) => { v.screen = 'endpoints'; v.draft.endpoints[2].dns = 'DNS_WRONG_TARGET'; v.draft.endpoints[2].seen = ['198.51.100.7']; });
  const tlsv = (bad) => ({ checks: [['PARSE', true], ['KEY_MATCH', !bad], ['NOT_EXPIRED', true], ['COVERS_NAMES', !bad], ['CHAIN', true], ['STRONG_SIGNATURE', !bad]], not_after: '2027-09-30T00:00:00Z', expiring_soon: false, names: ['os.empresa.test', '*.empresa.test'], uncovered: bad ? ['research.empresa.test'] : [] });
  set('tls-provided-valid', (v) => { v.screen = 'tls'; v.tls_validation = tlsv(false); });
  set('tls-invalid', (v) => { v.screen = 'tls'; v.tls_validation = tlsv(true); });
  set('tls-self-signed', (v) => { v.screen = 'tls'; v.draft.tls_mode = 'self'; });
  set('admin', (v) => { v.screen = 'admin'; });
  set('admin-invalid', (v) => { v.screen = 'admin'; v.draft.admin[3] = 'ana@empresa.test'; v.field_errors = { ae: 'ADMIN_NOT_DISTINCT' }; });
  set('review', (v) => { v.screen = 'review'; v.hardware = hw('one-gpu'); v.plan = plan(); });
  const ev = (at, phase, event, extra) => ({ at: '2026-10-04T' + at + 'Z', event: Object.assign({ event, phase }, extra || {}) });
  const EV = [ev('14:19:02', 'P06', 'StepCompleted'), ev('14:19:40', 'P10', 'StepWarning', { code: 'IMAGE_PULL_SLOW' }), ev('14:20:15', 'P10', 'StepCompleted'), ev('14:20:16', 'P11', 'StepStarted'), ev('14:20:21', 'P11', 'StepProgress', { operation: { op: 'create_instance' } })];
  set('installing', (v) => { v.screen = 'installing'; v.plan = plan(); v.busy = true; v.live = live(10, { P10: 'warn', P11: 'active' }, { warnings: 1, events: EV, ops: { P11: { op: 'create_instance' } } }); });
  set('cancel-safe-point', (v) => { v.screen = 'installing'; v.plan = plan(); v.busy = true; v.stopping = true; v.live = live(10, { P10: 'warn', P11: 'active' }, { ops: { P11: { op: 'create_instance' } } }); });
  set('interrupted', (v) => { v.screen = 'interrupted'; v.plan = plan(); v.live = live(8, { P09: 'active' }, { ops: { P09: { op: 'load_image', service: 'worker' } } }); });
  const failed = (name, n, phase, code, retryable, evs) => set(name, (v) => { v.screen = 'failed'; v.plan = plan(); v.ending = { outcome: 'FAILED', phase, code, retryable }; const x = {}; x[phase] = 'fail'; v.live = live(n, x, { events: evs || [] }); });
  failed('transfer-failed', 1, 'P02', 'TRANSFER_CHECKSUM_MISMATCH', true, [ev('14:12:09', 'P02', 'StepFailed', { code: 'TRANSFER_CHECKSUM_MISMATCH', detail: 'images/ocinye-core-server.tar' })]);
  failed('install-failed-migration', 10, 'P11', 'MIGRATION_FAILED', false, [ev('14:20:22', 'P11', 'StepFailed', { code: 'MIGRATION_FAILED', detail: 'migration 0041 refused' })]);
  failed('services-failed', 12, 'P13', 'SERVICE_UNHEALTHY:core', true, [ev('14:26:40', 'P13', 'StepFailed', { code: 'SERVICE_UNHEALTHY:core' })]);
  failed('admin-bootstrap-failed', 10, 'P11', 'ADMIN_BOOTSTRAP_REFUSED', false, [ev('14:20:30', 'P11', 'StepFailed', { code: 'ADMIN_BOOTSTRAP_REFUSED' })]);
  failed('prereq-failed', 3, 'P04', 'PREREQUISITE_INSTALL_FAILED', true, [ev('14:13:20', 'P04', 'StepFailed', { code: 'PREREQUISITE_INSTALL_FAILED', detail: 'apt docker-ce' })]);
  set('verifying', (v) => { v.screen = 'verifying'; v.plan = plan(); v.busy = true; v.live = live(14, { P15: 'active' }, { current: 'P15', verification: ver().items.slice(0, 6) }); });
  set('verify-failed', (v) => { v.screen = 'verifying'; v.plan = plan(); v.alert = { code: 'VERIFICATION_FAILED', params: {} }; v.verification = ver('fail'); v.credential = { user: 'ana.admin@empresa.test', expires_at: '2026-10-05 14:21 UTC' }; });
  const done = (name, mode) => set(name, (v) => { v.screen = 'complete'; v.plan = plan(mode === 'test' ? 'self' : null); v.hardware = hw('one-gpu'); v.verification = ver(mode); v.lifecycle = mode === 'dns' ? { state: 'ACTIVATION_PENDING', reasons: ['DNS'] } : mode === 'test' ? { state: 'INSTALLED_TEST_MODE' } : { state: 'OPERATIONAL' }; v.credential = { user: 'ana.admin@empresa.test', expires_at: '2026-10-05 14:21 UTC' }; });
  done('complete', 'ok');
  done('complete-test-mode', 'test');
  done('complete-dns-pending', 'dns');
  set('credential', (v) => { v.screen = 'credential'; v.plan = plan(); v.credential = { user: 'ana.admin@empresa.test', expires_at: '2026-10-05 14:21 UTC' }; });
  set('receipt', (v) => { v.screen = 'receipt'; v.plan = plan(); v.receipt = { schema: 1, installation_id: 'inst-7c41e20000000000', plan_id: 'pl-5d0a910000000000', plan_sha256: H('4'), release: '3f9c2a7d1e04', commit: '3f9c2a7d1e04b7c19e6a5d20f81c4e7a9b03d6f2', build: 'release', manifest_sha256: H('a'), artifacts: [{ path: 'source.tar.gz', sha256: H('7'), bytes: 1 }, { path: 'images/ocinye-core-server.tar', sha256: H('0'), bytes: 1 }, { path: 'install/ocinye', sha256: H('2'), bytes: 1 }], target_host: '192.0.2.10', host_key_sha256: FP, instance_name: 'Empresa Exemplo', instance_slug: 'empresa-exemplo', distributions: ['business', 'research'], endpoints: plan().configuration.endpoints, tls_mode: 'OPERATOR_SUPPLIED', tls_not_after: '2027-09-30T00:00:00Z', hardware: hw('one-gpu'), provider_mode: 'OFF', installed_packages: [], firewall_rules: ['80/tcp', '443/tcp'], warnings: [], started_at: '2026-10-04T14:16:31Z', ended_at: '2026-10-04T14:21:33Z', verification: ver(), lifecycle: { state: 'OPERATIONAL' }, first_access_pending: true }; });
  window.OI_VIEWS = V;
})();
