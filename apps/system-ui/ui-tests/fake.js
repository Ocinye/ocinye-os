/* Bancada da interface do sistema: um duplo dos dois serviços, com o mesmo
   formato de resposta do contrato. O cenário vem do fragmento do endereço:
   #signed-out (por omissão), #locked, #active, #offline, #mfa, #newpass, #down.
   Serve para ver e medir a interface num browser; não prova nada sobre os
   serviços. */
(function () {
  'use strict';
  const scenario = (location.hash || '#signed-out').slice(1);
  const PASSWORD = 'palavra-passe-de-prova';
  const F = window.SX_FAKE_STATE = {
    session: scenario === 'locked' ? 'LOCKED' : scenario === 'active' || scenario === 'offline' ? 'ACTIVE' : 'SIGNED_OUT',
    core: scenario === 'offline' ? 'UNREACHABLE' : 'ONLINE',
    failures: 0,
    calls: [],
    surface: null,
    a11y: { text_scale_percent: 100, high_contrast: false, reduce_motion: false },
    locale: { language: 'pt', system_locale: 'pt_PT.UTF-8' },
    privacy: { microphone_allowed: true, camera_allowed: true, microphone_in_use: false, camera_in_use: false },
    remote: { ssh_enabled: true, ssh_active: true },
    brightness: 70,
    grants: {},
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
        if (r.password !== PASSWORD) return refused('INVALID_CREDENTIALS');
        if (scenario === 'newpass' && !r.new_password) return refused('PASSWORD_CHANGE_REQUIRED');
        if (scenario === 'newpass' && r.new_password === 'palavra-passe-conhecida') return refused('PASSWORD_NOT_ACCEPTED');
        if (scenario === 'mfa' && !r.second_factor) return refused('SECOND_FACTOR_REQUIRED');
        if (scenario === 'mfa' && r.second_factor !== '123456') return refused('INVALID_CREDENTIALS');
        F.session = 'ACTIVE';
        return { reply: { reply: 'state', state: state() } };
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
    'system.capabilities': () => ok('inventory', { hardware: [{ hardware: 'NETWORK_INTERFACE', availability: { state: 'PRESENT' } }, { hardware: 'WIFI_RADIO', availability: { state: 'ABSENT' } }, { hardware: 'BACKLIGHT', availability: { state: 'PRESENT' } }, { hardware: 'BATTERY', availability: { state: 'PRESENT' } }], implemented: [] }),
    'network.status': () => ok('network', { interfaces: [{ name: 'enp1s0', kind: 'ETHERNET', carrier: true, mac: '52:54:00:12:34:56', addresses: ['192.168.1.84/24'], speed_mbps: 1000 }], gateway: '192.168.1.1', dns: ['192.168.1.1'], connectivity: 'LOCAL', wifi: null }),
    'audio.status': () => no('NOT_IMPLEMENTED'),
    'display.status': () => ok('display', { outputs: [{ name: 'eDP-1', internal: true, width: 1920, height: 1080, refresh_mhz: 60000, scale_percent: 100 }], brightness: F.brightness }),
    'power.status': () => ok('power', { source: 'BATTERY', battery: { charge: 82, charging: false, health: 96 }, can_suspend: true, inhibitors: [] }),
    'locale.status': () => ok('locale', F.locale),
    'time.status': () => ok('time', { timezone: 'Europe/Lisbon', automatic: true, synchronised: true, unix_time: 1791680000 }),
    'keyboard.status': () => ok('keyboard', { layout: 'pt', variant: null, offered: ['pt', 'us', 'fr', 'gb', 'es', 'de'] }),
    'accessibility.status': () => ok('accessibility', F.a11y),
    'privacy.status': () => ok('privacy', F.privacy),
    'storage.status': () => ok('storage', { disks: [{ name: 'nvme0n1', model: 'Disco de prova', size_bytes: 512e9, removable: false }], volumes: [{ mount_point: '/', filesystem: 'ext4', total_bytes: 60e9, free_bytes: 41e9 }, { mount_point: '/var/lib/ocinye', filesystem: 'ext4', total_bytes: 430e9, free_bytes: 402e9 }] }),
    'update.status': () => ok('update', { current_release: 'ocinye-os-1.0.0-dev', pending_release: null, phase: 'UP_TO_DATE', signing: 'DEV_ONLY' }),
    'remote_access.status': () => ok('remote_access', F.remote),
    'recovery.status': () => ok('recovery', { rollback_available: false, previous_release: null }),
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
      case 'privacy.set_microphone': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.privacy = Object.assign({}, F.privacy, { microphone_allowed: request.allowed }); return ok('privacy', F.privacy);
      case 'privacy.set_camera': if (!member) return no('NOT_AUTHORIZED', { needs: 'MEMBER' }); F.privacy = Object.assign({}, F.privacy, { camera_allowed: request.allowed }); return ok('privacy', F.privacy);
      case 'remote_access.configure': { const f = fresh('LOCAL_REAUTH'); if (f) return f; F.remote = { ssh_enabled: request.ssh_enabled, ssh_active: request.ssh_enabled }; return ok('remote_access', F.remote); }
      case 'power.restart': case 'power.shutdown': { if (member) { const f = fresh('CONFIRM'); if (f) return f; } return ok('done'); }
      case 'power.suspend': return ok('done');
      case 'time.configure': return no('AUTHORITY_UNAVAILABLE');
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
