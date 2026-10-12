/* Bancada do instalador: um motor de faz-de-conta que fala o contrato da
   janela (ocinye_image_contracts::window) — passos tipados para lá, acções
   tipadas para cá — com discos de todos os géneros e um modelo com marcação
   hostil. O cenário vem do fragmento: #ok (omissão), #blocked, #fail,
   #elsewhere, #key; e as sessões que não instalam: #live, #hardware, #recovery. Não instala nada, e não prova nada sobre o motor. */
(function () {
  'use strict';
  const scenario = (location.hash || '#ok').slice(1);
  const disk = (device, model, serial, bytes, protection, partitions, filesystems) => ({ device, by_path: '/dev/disk/by-path/' + device, bytes, model, serial, wwn: null, partitions, filesystems, protection, confirmation_token: serial ? serial.slice(-4) : device });
  const DISKS = [
    disk('sr0', 'QEMU CD-ROM', 'cd', 5.0e9, 'INSTALLATION_MEDIA', 2, ['iso9660']),
    disk('nvme0n1', 'Samsung SSD 990 <img src=x onerror=alert(1)>', 'OCY-TARGET-7F3A', 512.1e9, 'ELIGIBLE', 0, []),
    disk('sda', 'WDC WD40EFRX', 'WD-9K2M', 4000.8e9, 'ELIGIBLE', 2, ['ntfs', 'ext4']),
    disk('sdb', '', null, 64.0e9, 'AMBIGUOUS', 1, []),
    disk('sdc', 'Kingston DataTraveler', 'SMALL-0001', 8.0e9, 'TOO_SMALL', 1, ['vfat']),
  ];
  const F = window.OI_FAKE_STATE = { out: [], actions: [], at: 'WELCOME', disk: null, wrote: null, settings: { keyboard: 'pt', network: { mode: 'AUTOMATIC' }, hostname: null } };
  const LAYOUTS = [['pt', 'Português (Portugal)'], ['br', 'Português (Brasil)'], ['us', 'English (US)'], ['gb', 'English (UK)'], ['fr', 'Français'], ['es', 'Español'], ['de', 'Deutsch'], ['it', 'Italiano'], ['ch', 'Suisse / Schweiz'], ['be', 'Belgique']].map(([code, name]) => ({ code, name }));
  const IFACES = [{ name: 'enp1s0', wired: true, carrier: true, mac: '52:54:00:12:34:56' }, { name: 'wlp2s0', wired: false, carrier: false, mac: '52:54:00:ab:cd:ef' }];
  const keyboard = () => step({ step: 'KEYBOARD', layouts: LAYOUTS, chosen: F.settings.keyboard });
  const network = (refused) => step({ step: 'NETWORK', interfaces: IFACES, settings: F.settings.network, refused: refused || null });
  const advanced = (refused) => step({ step: 'ADVANCED', hostname: F.settings.hostname, layout: { efi_bytes: 1073741824, filesystem: 'ext4', swap: false }, refused: refused || null });
  const review = () => step({ step: 'REVIEW', disk: F.disk, settings: F.settings, confirmation_hint: F.disk.serial || F.disk.confirmation_token, confirmation_length: F.disk.confirmation_token.length });
  const ip = (v) => /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.test(v) && v.split('.').every((n) => Number(n) <= 255);
  const step = (state) => { F.at = state.step; F.out.push({ event: 'step', state }); };
  const storage = (refused) => step({ step: 'STORAGE', disks: DISKS, minimum_bytes: 20e9, refused: refused || null });
  F.out.push({ event: 'hello', protocol: 2 });
  // As sessões que não instalam: o relatório só de leitura, e mais nada.
  const MODES = { live: 'live', hardware: 'hardware-check', recovery: 'recovery' };
  const report = () => ({
    arch: 'x86_64', machine: scenario === 'recovery' ? null : 'LENOVO 20L8S02E00 <b>x</b>', cpu_model: 'Intel(R) Core(TM) i5-8350U CPU @ 1.70GHz', cpus: 8, memory_bytes: 16 * 1073741824,
    uefi: true, secure_boot: scenario === 'hardware' ? true : null, firmware: scenario === 'recovery' ? null : 'LENOVO N24ET75W (1.50)',
    disks: [{ device: 'sda', bytes: 15.5e9, model: 'SanDisk Ultra', connection: 'usb', boot_media: true }, { device: 'nvme0n1', bytes: 512.1e9, model: 'Samsung SSD 990', connection: 'nvme', boot_media: false }],
    network: [{ name: 'enp0s31f6', wired: true, carrier: true, addresses: ['192.168.1.37/24', 'fe80::1/64'] }, { name: 'wlp2s0', wired: false, carrier: false, addresses: [] }],
    wifi_radios: 1, bluetooth_controllers: scenario === 'hardware' ? 1 : null, displays: ['eDP-1', 'HDMI-A-1'], audio: scenario === 'recovery' ? null : ['HDA Intel PCH'],
  });
  const media = () => { const r = report(); step({ step: 'MEDIA', mode: MODES[scenario], release_id: '9d2c3d2197b8-r1', development: true, language: 'pt', restricted: scenario === 'recovery', report: r, compatibility: { uefi: true, architecture_supported: true, memory_required: 2 * 1073741824, minimum_disk_bytes: 20e9, large_enough_disks: 1 } }); };
  if (MODES[scenario]) media();
  else step({ step: 'WELCOME', release_id: '9d2c3d2197b8', development: true, language: 'pt' });

  function install() {
    step({ step: 'INSTALLING' });
    const plan = [['media_check', true], ['install', scenario !== 'fail'], ['verify', true], ['first_boot', true]];
    let i = 0;
    const tick = () => {
      const [name, ok] = plan[i];
      F.out.push({ event: 'detail', line: `[${ok ? 'OK' : 'DISK_WRITE_FAILED'}] ${name}` });
      F.out.push({ event: 'stage', name, ok });
      if (!ok) return step({ step: 'FAILED', error: { code: 'DISK_WRITE_FAILED', device: F.disk.device }, disk_touched: true });
      i += 1;
      if (i < plan.length) setTimeout(tick, 350); else { F.wrote = F.disk.by_path; step({ step: 'INSTALLED' }); }
    };
    setTimeout(tick, 350);
  }

  function act(a) {
    F.actions.push(a);
    if (F.at === 'MEDIA') {
      if (a.action === 'rescan') media();
      if (a.action === 'restart' || a.action === 'power_off') F.ended = a.action;
      return;
    }
    const right = () => F.disk && a.typed.trim().toUpperCase() === F.disk.confirmation_token.toUpperCase();
    if (F.at === 'WELCOME' || F.at === 'DRIVEN_ELSEWHERE') {
      if (a.action !== 'begin') return;
      if (scenario === 'elsewhere') return step({ step: 'DRIVEN_ELSEWHERE' });
      const blocked = scenario === 'blocked' ? { code: 'STORAGE_PROTECTION_UNVERIFIED', reason: 'swap_active' } : null;
      return step({ step: 'COMPATIBILITY', uefi: true, architecture_supported: true, memory_bytes: 16 * 1073741824, memory_required: 2 * 1073741824, storage_protected: !blocked, blocked });
    }
    if (F.at === 'COMPATIBILITY') { if (a.action === 'continue' && scenario !== 'blocked') keyboard(); return; }
    if (F.at === 'KEYBOARD') {
      if (a.action === 'choose_keyboard' && LAYOUTS.some((l) => l.code === a.layout)) F.settings.keyboard = a.layout;
      if (a.action === 'continue') return network();
      return keyboard();
    }
    if (F.at === 'NETWORK') {
      if (a.action === 'back') return keyboard();
      if (a.action === 'continue') return storage();
      if (a.action === 'set_network') {
        const n = a.settings;
        if (n.mode === 'STATIC' && !IFACES.some((i) => i.name === n.interface)) return network('UNKNOWN_INTERFACE');
        if (n.mode === 'STATIC' && (!ip(n.address) || !ip(n.gateway) || !(n.prefix >= 1 && n.prefix <= 32) || !n.dns.every(ip))) return network('INVALID_ADDRESS');
        // The gateway on the address's own network, as the engine checks.
        const num = (v) => v.split('.').reduce((a, n) => a * 256 + Number(n), 0);
        const mask = n.prefix >= 32 ? 0xffffffff : (0xffffffff - (2 ** (32 - n.prefix) - 1));
        if (n.mode === 'STATIC' && n.prefix < 31 && Math.floor(num(n.address) / 2 ** (32 - n.prefix)) !== Math.floor(num(n.gateway) / 2 ** (32 - n.prefix))) return network('INVALID_ADDRESS');
        void mask;
        F.settings.network = n;
      }
      return network();
    }
    if (F.at === 'ADVANCED') {
      if (a.action === 'back') return storage();
      if (a.action === 'continue') return review();
      if (a.action === 'set_hostname') {
        if (a.hostname && !/^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(a.hostname)) return advanced('INVALID_HOSTNAME');
        F.settings.hostname = a.hostname;
      }
      return advanced();
    }
    if (F.at === 'STORAGE') {
      if (a.action === 'rescan') return storage();
      if (a.action === 'back') return network();
      if (a.action !== 'select_disk') return;
      const d = DISKS.find((x) => x.by_path === a.by_path);
      if (!d) return storage({ code: 'AMBIGUOUS_DISK' });
      if (d.protection !== 'ELIGIBLE' && d.protection !== 'AMBIGUOUS') return storage({ code: 'DISK_PROTECTED', protection: d.protection });
      F.disk = d;
      return advanced();
    }
    if (F.at === 'STORAGE' && a.action === 'back') return network();
    if (F.at === 'REVIEW') {
      if (a.action === 'back') return advanced();
      if (a.action === 'check_confirmation') return void F.out.push({ event: 'confirmation_checked', valid: right() });
      if (a.action === 'confirm') {
        if (!right()) return void F.out.push({ event: 'confirmation_checked', valid: false });
        if (scenario === 'key') return step({ step: 'KEY_OFFER', fingerprint: 'SHA256:9fK2bancadaQm7dXw' });
        return install();
      }
      return;
    }
    if (F.at === 'KEY_OFFER' && a.action === 'answer_key') return install();
  }

  window.OI_FAKE = async (cmd, args) => {
    // Como na máquina: o motor ainda não está lá nas primeiras perguntas, e a
    // acção de começar só aparece depois do primeiro desenho.
    if (cmd === 'installer_poll') {
      F.polls = (F.polls || 0) + 1;
      if (F.polls <= 2) return { state: 'WAITING', events: [] };
      const events = F.out; F.out = []; return { state: 'OPEN', events };
    }
    if (cmd === 'installer_send') { act(args.action); return true; }
    return null;
  };
})();
