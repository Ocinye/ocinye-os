/* Bancada da interface do sistema: um duplo dos dois serviços, com o mesmo
   formato de resposta do contrato. O cenário vem do fragmento do endereço:
   #signed-out (por omissão), #locked, #active, #offline, #mfa, #newpass, #enroll, #setup, #down.
   Serve para ver e medir a interface num browser; não prova nada sobre os
   serviços. */
(function () {
  'use strict';
  const scenario = (location.hash || '#signed-out').slice(1);
  const PASSWORD = 'palavra-passe-de-prova';
  // O que o duplo entrega como credencial temporária: um valor que se vê à
  // légua que não é de ninguém.
  const FIRST_CREDENTIAL = ['temporaria', 'de', 'prova'].join(' ');
  const F = window.SX_FAKE_STATE = {
    session: scenario === 'locked' ? 'LOCKED' : scenario === 'active' || scenario === 'offline' ? 'ACTIVE' : 'SIGNED_OUT',
    core: scenario === 'offline' ? 'UNREACHABLE' : 'ONLINE',
    failures: 0,
    calls: [],
    surface: null,
    a11y: { text_scale_percent: 100, high_contrast: false, reduce_motion: false },
    locale: { language: 'pt', system_locale: 'pt_PT.UTF-8' },
    privacy: { microphone_present: true, camera_present: true, microphone_allowed: true, camera_allowed: true, microphone_in_use: false, camera_in_use: false },
    // Wi-Fi: o rádio, a rede em uso e as redes ao alcance (a palavra-passe
    // que o duplo aceita é a de baixo; outra é recusada como uma rede faria).
    wifi: { enabled: true, connected: 'Laboratório' },
    networks: [
      { ssid: 'Laboratório', security: 'PASSPHRASE', signal: 82, known: true, connected: true },
      { ssid: 'Biblioteca', security: 'PASSPHRASE', signal: 61, known: false, connected: false },
      { ssid: 'Visitantes', security: 'OPEN', signal: 47, known: false, connected: false },
      { ssid: 'Empresa-802.1X', security: 'ENTERPRISE', signal: 30, known: false, connected: false },
    ],
    wifiPass: 'palavra-passe-wifi',
    bt: { powered: true, devices: [
      { address: '00:1A:7D:DA:71:13', name: 'Teclado de prova', class: 'KEYBOARD', paired: true, connected: true },
      { address: '00:1A:7D:DA:71:14', name: 'Auscultadores de prova', class: 'AUDIO', paired: false, connected: false },
    ] },
    audio: { output: 0, outVolume: 60, outMuted: false, inVolume: 70, inMuted: false },
    remote: { ssh_enabled: true, ssh_active: true },
    brightness: 70,
    grants: {},
    activation: scenario === 'setup' ? 'READY' : 'ACTIVATED',
    credential: false,
    steps: 0,
    // The Update Engine, as the interface sees it (ADR-0032). `?upd=` picks
    // where it starts: available, downloading, awaiting, reboot, rolledback,
    // recovery, failed, norepo; otherwise up to date.
    upd: new URLSearchParams(location.search).get('upd') || 'idle',
    updDone: 0,
  };
  const CUR = { version: 'd34bbb0a141d-r1', release_number: 7, channel: 'development', activated_at: '2026-10-10T09:12:00Z' };
  const PREV = { version: '2916a481e178-r1', release_number: 6, channel: 'development', activated_at: '2026-10-02T15:40:00Z' };
  const NEXT = { version: 'e81c2f90b7aa-r1', release_number: 8, download_bytes: 642e6, install_bytes: 1284e6, reboot_required: false, security: true, migrations: { kind: 'expand' }, notes: { pt: 'Actualizações do Workspace e correcções de segurança da base de dados.', en: 'Workspace updates and database security fixes.', fr: 'Mises à jour du Workspace et correctifs de sécurité de la base de données.' }, synthetic: false, source: 'repository' };
  const PHASE = { idle: 'IDLE', available: 'UPDATE_AVAILABLE', downloading: 'DOWNLOADING', awaiting: 'AWAITING_INSTALL', installing: 'INSTALLING', reboot: 'AWAITING_REBOOT', rolledback: 'ROLLED_BACK', recovery: 'RECOVERY_REQUIRED', failed: 'DOWNLOAD_FAILED', norepo: 'IDLE', completed: 'COMPLETED' };
  const updState = () => {
    if (F.upd === 'downloading') { F.updDone = Math.min(NEXT.download_bytes, F.updDone + 160e6); if (F.updDone >= NEXT.download_bytes) F.upd = 'awaiting'; }
    if (F.upd === 'installing') { F.steps += 1; if (F.steps > 2) { F.upd = 'completed'; } }
    const p = PHASE[F.upd];
    const done = F.upd === 'completed';
    return {
      current: done ? { version: NEXT.version, release_number: 8, channel: 'development', activated_at: '2026-10-12T14:00:00Z' } : CUR,
      previous: done ? CUR : PREV,
      channel: 'development', repository_configured: F.upd !== 'norepo', phase: p,
      available: ['available', 'downloading', 'awaiting', 'installing', 'reboot', 'failed'].includes(F.upd) ? Object.assign({}, NEXT, { reboot_required: F.upd === 'reboot' || NEXT.reboot_required }) : null,
      progress: F.upd === 'downloading' ? { done: F.updDone, total: NEXT.download_bytes } : null,
      last_check: '2026-10-12T13:58:00Z', last_outcome: done ? 'COMMITTED' : F.upd === 'rolledback' ? 'ROLLED_BACK' : F.upd === 'recovery' ? 'RECOVERY_REQUIRED' : null,
      reason: F.upd === 'rolledback' ? 'HEALTH_GATE_FAILED' : F.upd === 'recovery' ? 'ROLLBACK_UNSAFE' : F.upd === 'failed' ? 'DOWNLOAD_INTERRUPTED' : null,
      restart_required: F.upd === 'reboot', signing: 'DEV_ONLY', auto_check: false, auto_download: false,
      offline_bundles: [{ name: 'ocinye-os-e81c2f90b7aa-r1.ocu', bytes: 642e6, removable: true }],
    };
  };
  const state = () => F.session === 'ACTIVE'
    ? { state: 'ACTIVE', member_id: 'm-1', member_name: 'Pessoa de Prova', core: F.core, operator: true, idle_lock_secs: 300 }
    : F.session === 'LOCKED'
      ? { state: 'LOCKED', member_name: 'Pessoa de Prova', core: F.core, retry_after_secs: F.failures > 2 ? 4 : 0 }
      : { state: 'SIGNED_OUT', ended: null };
  const refused = (code, more) => ({ reply: { reply: 'refused', reason: Object.assign({ code }, more || {}) } });

  function session(r) {
    switch (r.request) {
      case 'state': return { reply: { reply: 'state', state: state() } };
      case 'sign_in':
        if (r.password !== PASSWORD && !(scenario === 'enroll' && F.newset)) return refused('INVALID_CREDENTIALS');
        if (scenario === 'newpass' && !r.new_password) return refused('PASSWORD_CHANGE_REQUIRED');
        if (scenario === 'newpass' && r.new_password === 'palavra-passe-conhecida') return refused('PASSWORD_NOT_ACCEPTED');
        if (scenario === 'enroll') {
          if (!F.newset && !r.new_password) return refused('PASSWORD_CHANGE_REQUIRED');
          if (r.new_password) { F.newset = r.new_password; return refused('SECOND_FACTOR_ENROLLMENT_REQUIRED'); }
          if (r.password !== F.newset) return refused('INVALID_CREDENTIALS');
          if (!r.second_factor) return refused('SECOND_FACTOR_ENROLLMENT_REQUIRED');
          if (r.second_factor !== '123456') return refused('INVALID_CREDENTIALS');
          F.session = 'ACTIVE';
          return { reply: { reply: 'enrolled', state: state(), recovery_codes: ['aaaa-bbbb-cccc', 'dddd-eeee-ffff', 'gggg-hhhh-iiii', 'jjjj-kkkk-llll'] } };
        }
        if (scenario === 'mfa' && !r.second_factor) return refused('SECOND_FACTOR_REQUIRED');
        if (scenario === 'mfa' && r.second_factor !== '123456') return refused('INVALID_CREDENTIALS');
        F.session = 'ACTIVE';
        return { reply: { reply: 'state', state: state() } };
      case 'begin_second_factor_enrollment': return r.password === (F.newset || PASSWORD) ? { reply: { reply: 'second_factor_secret', secret_base32: 'AAAABBBBCCCCDDDDAAAABBBBCCCCDDDD' } } : refused('INVALID_CREDENTIALS');
      case 'lock': if (F.session === 'ACTIVE') F.session = 'LOCKED'; return { reply: { reply: 'state', state: state() } };
      case 'unlock':
        if (r.password !== PASSWORD) { F.failures += 1; return refused('INVALID_CREDENTIALS'); }
        F.failures = 0; F.session = 'ACTIVE';
        return { reply: { reply: 'state', state: state() } };
      case 'sign_out': F.session = 'SIGNED_OUT'; return { reply: { reply: 'state', state: state() } };
      case 'confirm': { const g = 'g' + Math.random(); F.grants[g] = r.binding; return { reply: { reply: 'granted', grant: g } }; }
      case 'reauthenticate': {
        if (r.password !== PASSWORD) return refused('INVALID_CREDENTIALS');
        const g = 'g' + Math.random(); F.grants[g] = r.binding; return { reply: { reply: 'granted', grant: g } };
      }
      default: return refused('MALFORMED');
    }
  }

  const ok = (kind, value) => ({ outcome: { outcome: 'OK', payload: value === undefined ? { kind } : { kind, value } } });
  const no = (code, more) => ({ outcome: { outcome: 'REFUSED', reason: Object.assign({ code }, more || {}) } });
  const READS = {
    'system.health': () => ok('health', { items: [{ part: 'system_service', health: 'OK' }, { part: 'network', health: 'OK' }, { part: 'clock', health: 'OK' }, { part: 'storage', health: 'OK' }, { part: 'instance', health: F.core === 'ONLINE' ? 'OK' : 'FAILED' }] }),
    'system.info': () => ok('info', { hostname: 'ocinye-01', release_id: 'ocinye-os-1.0.0-dev', development_build: true, base_system: 'Ubuntu 24.04.3 LTS', kernel: '6.8.0-90-generic', arch: 'x86_64', cpu_model: 'Processador de prova', cpu_count: 8, memory_mib: 16000, uefi: true, secure_boot: 'DISABLED' }),
    'system.capabilities': () => ok('inventory', { hardware: [{ hardware: 'NETWORK_INTERFACE', availability: { state: 'PRESENT' } }, { hardware: 'WIFI_RADIO', availability: { state: 'PRESENT' } }, { hardware: 'BLUETOOTH_CONTROLLER', availability: { state: 'PRESENT' } }, { hardware: 'AUDIO_OUTPUT', availability: { state: 'PRESENT' } }, { hardware: 'BACKLIGHT', availability: { state: 'PRESENT' } }, { hardware: 'BATTERY', availability: { state: 'PRESENT' } }], implemented: [] }),
    'network.status': () => ok('network', { interfaces: [{ name: 'enp1s0', kind: 'ETHERNET', carrier: true, mac: '52:54:00:12:34:56', addresses: ['192.168.1.84/24'], speed_mbps: 1000 }, { name: 'wlp2s0', kind: 'WIFI', carrier: !!F.wifi.connected, mac: '52:54:00:ab:cd:ef', addresses: F.wifi.connected ? ['192.168.1.85/24'] : [], speed_mbps: null }], gateway: '192.168.1.1', dns: ['192.168.1.1'], connectivity: 'LOCAL', wifi: { enabled: F.wifi.enabled, connected: F.wifi.enabled ? F.wifi.connected : null } }),
    'wifi.scan': () => (F.wifi.enabled ? ok('wifi_scan', { networks: F.networks.map((n) => Object.assign({}, n, { connected: n.ssid === F.wifi.connected })) }) : no('HARDWARE_UNAVAILABLE', { cause: 'SOFT_BLOCKED' })),
    'bluetooth.status': () => ok('bluetooth', F.bt),
    'audio.status': () => ok('audio', {
      outputs: [{ id: 0, name: 'HDA Intel PCH', default: F.audio.output === 0, volume: F.audio.output === 0 ? F.audio.outVolume : 50, muted: F.audio.output === 0 && F.audio.outMuted }, { id: 1, name: 'USB Audio Device', default: F.audio.output === 1, volume: F.audio.output === 1 ? F.audio.outVolume : 40, muted: F.audio.output === 1 && F.audio.outMuted }],
      inputs: [{ id: 0, name: 'HDA Intel PCH', default: F.audio.output === 0, volume: F.audio.inVolume, muted: F.audio.inMuted || !F.privacy.microphone_allowed }],
    }),
    'display.status': () => ok('display', { outputs: [{ name: 'eDP-1', internal: true, enabled: true, width: 1920, height: 1080, refresh_mhz: null, scale_percent: null }, { name: 'HDMI-A-1', internal: false, enabled: true, width: 2560, height: 1440, refresh_mhz: null, scale_percent: null }], brightness: F.brightness }),
    'power.status': () => ok('power', { source: 'BATTERY', battery: { charge: 82, charging: false, health: 96 }, can_suspend: true, inhibitors: [] }),
    'locale.status': () => ok('locale', F.locale),
    'time.status': () => ok('time', { timezone: 'Europe/Lisbon', automatic: true, synchronised: true, unix_time: 1791680000 }),
    'keyboard.status': () => ok('keyboard', { layout: 'pt', variant: null, offered: ['pt', 'us', 'fr', 'gb', 'es', 'de'] }),
    'accessibility.status': () => ok('accessibility', F.a11y),
    'privacy.status': () => ok('privacy', F.privacy),
    'storage.status': () => ok('storage', { disks: [{ name: 'nvme0n1', model: 'Disco de prova', size_bytes: 512e9, removable: false }], volumes: [{ mount_point: '/', filesystem: 'ext4', total_bytes: 60e9, free_bytes: 41e9 }, { mount_point: '/var/lib/ocinye', filesystem: 'ext4', total_bytes: 430e9, free_bytes: 402e9 }] }),
    'update.status': () => ok('update', updState()),
    'update.history': () => ok('update_history', { entries: [{ id: '5f1e0c2a9b7d3e41', version: CUR.version, release_number: 7, channel: 'development', from: PREV.version, started_at: '2026-10-10T09:02:00Z', finished_at: '2026-10-10T09:12:00Z', outcome: 'COMMITTED', reason: null, rebooted: false, source: 'repository', via: 'settings' }, { id: '0c9d8e7f6a5b4c3d', version: 'aa11bb22cc33-r1', release_number: 6, channel: 'development', from: PREV.version, started_at: '2026-10-08T11:00:00Z', finished_at: '2026-10-08T11:09:00Z', outcome: 'ROLLED_BACK', reason: 'HEALTH_GATE_FAILED', rebooted: false, source: 'offline', via: 'cli' }] }),
    'base.status': () => ok('base', { phase: 'IDLE', sources_approved: true, pending: 12, security: 3, last_check: '2026-10-12T06:00:00Z', reboot_required: false, last_outcome: null, reason: null, major_upgrade_supported: false }),
    'remote_access.status': () => ok('remote_access', F.remote),
    'activation.status': () => { if (F.activation === 'RUNNING') { F.steps += 1; if (F.steps > 3) { F.activation = 'ACTIVATED'; F.credential = true; } } return ok('activation', { phase: F.activation, release_id: 'ocinye-os-1.0.0-dev', steps_begun: F.activation === 'RUNNING' ? F.steps : 0, credential_waiting: F.credential }); },
    'recovery.status': () => ok('recovery', { rollback_available: true, previous_release: PREV.version, current_release: CUR.version, pending_release: null, last_update: null }),
  };

  function system(request, grant) {
    const cap = request.capability;
    if (scenario === 'down') return { unreachable: 'SYSTEM_SERVICE_UNREACHABLE' };
    if (READS[cap]) return READS[cap]();
    const member = F.session === 'ACTIVE';
    const binding = JSON.stringify(request);
    const fresh = (cls) => { if (grant && F.grants[grant] === binding) { delete F.grants[grant]; return null; } return no(grant ? 'GRANT_INVALID' : 'FRESH_AUTH_REQUIRED', grant ? {} : { class: cls }); };
    switch (cap) {
      case 'accessibility.configure': F.a11y = { text_scale_percent: request.text_scale_percent, high_contrast: request.high_contrast, reduce_motion: request.reduce_motion }; return ok('accessibility', F.a11y);
      case 'locale.configure': F.locale = { language: request.language, system_locale: F.locale.system_locale }; return ok('locale', F.locale);
      case 'display.set_brightness': F.brightness = request.percent; return READS['display.status']();
      case 'keyboard.configure': return member ? ok('keyboard', { layout: request.layout, variant: null, offered: ['pt', 'us', 'fr', 'gb', 'es', 'de'] }) : no('NOT_AUTHORIZED', { needs: 'MEMBER' });
      case 'privacy.set_microphone': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.privacy = Object.assign({}, F.privacy, { microphone_allowed: request.allowed }); F.audio.inMuted = !request.allowed; return ok('privacy', F.privacy);
      case 'wifi.set_radio': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.wifi = { enabled: request.enabled, connected: request.enabled ? F.wifi.connected : null }; return READS['network.status']();
      case 'wifi.connect': {
        if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' });
        const n = F.networks.find((x) => x.ssid === request.ssid);
        if (!n) return no('NOT_FOUND');
        if (n.security === 'ENTERPRISE') return no('NOT_IMPLEMENTED');
        if (n.security === 'PASSPHRASE' && !n.known && request.passphrase !== F.wifiPass) return no(request.passphrase ? 'WIFI_AUTHENTICATION_FAILED' : 'VALUE_OUT_OF_RANGE');
        if (n.security === 'PASSPHRASE') n.known = true;
        F.wifi.connected = n.ssid; return READS['network.status']();
      }
      case 'wifi.disconnect': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.wifi.connected = null; return READS['network.status']();
      case 'wifi.forget': {
        if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' });
        const f = fresh('CONFIRM'); if (f) return f;
        const n = F.networks.find((x) => x.ssid === request.ssid && x.known);
        if (!n) return no('NOT_FOUND');
        n.known = false; if (F.wifi.connected === n.ssid) F.wifi.connected = null; return READS['network.status']();
      }
      case 'network.configure': { const f = fresh('LOCAL_REAUTH'); if (f) return f; return READS['network.status'](); }
      case 'bluetooth.set_power': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.bt.powered = request.powered; if (!request.powered) F.bt.devices.forEach((d) => { d.connected = false; }); return READS['bluetooth.status']();
      case 'bluetooth.scan':
        if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' });
        if (!F.bt.powered) return no('HARDWARE_UNAVAILABLE', { cause: 'SOFT_BLOCKED' });
        if (!F.bt.devices.some((d) => d.address === '00:1A:7D:DA:71:15')) F.bt.devices.push({ address: '00:1A:7D:DA:71:15', name: 'Rato de prova', class: 'POINTER', paired: false, connected: false });
        return READS['bluetooth.status']();
      case 'bluetooth.pair': case 'bluetooth.connect': case 'bluetooth.disconnect': case 'bluetooth.remove': {
        if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' });
        if (cap === 'bluetooth.pair' || cap === 'bluetooth.remove') { const f = fresh('CONFIRM'); if (f) return f; }
        const d = F.bt.devices.find((x) => x.address === request.address);
        if (!d) return no('NOT_FOUND');
        // O duplo recusa os auscultadores como um dispositivo que pede código.
        if (cap === 'bluetooth.pair' && d.class === 'AUDIO') return no('ADAPTER_FAILED', { cause: 'REFUSED' });
        if (cap === 'bluetooth.pair') d.paired = true;
        if (cap === 'bluetooth.connect') d.connected = true;
        if (cap === 'bluetooth.disconnect') d.connected = false;
        if (cap === 'bluetooth.remove') F.bt.devices = F.bt.devices.filter((x) => x !== d);
        return READS['bluetooth.status']();
      }
      case 'audio.set_volume': if (request.target === 'OUTPUT') F.audio.outVolume = request.percent; else F.audio.inVolume = request.percent; return READS['audio.status']();
      case 'audio.set_mute':
        if (request.target === 'INPUT' && !request.muted && !F.privacy.microphone_allowed) return no('HARDWARE_UNAVAILABLE', { cause: 'SOFT_BLOCKED' });
        if (request.target === 'OUTPUT') F.audio.outMuted = request.muted; else F.audio.inMuted = request.muted;
        return READS['audio.status']();
      case 'audio.set_output': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); if (request.id !== 0 && request.id !== 1) return no('NOT_FOUND'); F.audio.output = request.id; return READS['audio.status']();
      case 'privacy.set_camera': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.privacy = Object.assign({}, F.privacy, { camera_allowed: request.allowed }); return ok('privacy', F.privacy);
      case 'remote_access.configure': { const f = fresh('LOCAL_REAUTH'); if (f) return f; F.remote = { ssh_enabled: request.ssh_enabled, ssh_active: request.ssh_enabled }; return ok('remote_access', F.remote); }
      case 'power.restart': case 'power.shutdown': { if (member) { const f = fresh('CONFIRM'); if (f) return f; } return ok('done'); }
      case 'power.suspend': return ok('done');
      case 'update.check': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); if (F.upd === 'norepo') return no('UPDATE_REFUSED', { update: 'REPOSITORY_NOT_CONFIGURED' }); F.upd = 'available'; return READS['update.status']();
      case 'update.download': F.upd = 'downloading'; F.updDone = 0; return READS['update.status']();
      case 'update.cancel': F.upd = 'available'; return READS['update.status']();
      case 'update.install': { const f = fresh('LOCAL_REAUTH'); if (f) return f; F.upd = 'installing'; F.steps = 0; return READS['update.status'](); }
      case 'update.rollback': { const f = fresh('LOCAL_REAUTH'); if (f) return f; F.upd = 'rolledback'; return READS['update.status'](); }
      case 'update.import': F.upd = 'available'; return READS['update.status']();
      case 'update.configure': { const f = fresh('LOCAL_REAUTH'); if (f) return f; return READS['update.status'](); }
      case 'base.check': return READS['base.status']();
      case 'base.apply': { const f = fresh('LOCAL_REAUTH'); if (f) return f; return READS['base.status'](); }
      case 'time.configure': return no('AUTHORITY_UNAVAILABLE');
      case 'activation.start': if (F.activation !== 'READY' && F.activation !== 'FAILED') return no('NOT_OFFERED_HERE'); if (!/^[a-z0-9-]+(\.[a-z0-9-]+)+$/.test(request.domain)) return no('VALUE_OUT_OF_RANGE'); F.activation = 'RUNNING'; F.steps = 0; return READS['activation.status']();
      case 'activation.credential': if (!F.credential) return no('NOT_FOUND'); F.credential = false; return ok('initial_credential', { username: 'admin@empresa.example', password: FIRST_CREDENTIAL });
      default: return no('NOT_IMPLEMENTED');
    }
  }

  window.SX_FAKE = async (cmd, args) => {
    F.calls.push([cmd, JSON.parse(JSON.stringify(args))]);
    if (cmd === 'session') return session(args.request);
    if (cmd === 'system') return system(args.request, args.grant);
    if (cmd === 'binding') return JSON.stringify(args.request);
    if (cmd === 'surface') { F.surface = args.full ? 'full' : 'bar'; return true; }
    if (cmd === 'open_workspace') return true;
    return null;
  };
})();
