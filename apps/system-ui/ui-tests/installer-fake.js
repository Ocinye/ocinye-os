/* Bancada do instalador: um motor de faz-de-conta que fala o contrato da
   janela (ocinye_image_contracts::window) — passos tipados para lá, acções
   tipadas para cá — com discos de todos os géneros e um modelo com marcação
   hostil. O cenário vem do fragmento: #ok (omissão), #blocked, #fail,
   #elsewhere, #key. Não instala nada, e não prova nada sobre o motor. */
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
  const F = window.OI_FAKE_STATE = { out: [], actions: [], at: 'WELCOME', disk: null, wrote: null };
  const step = (state) => { F.at = state.step; F.out.push({ event: 'step', state }); };
  const storage = (refused) => step({ step: 'STORAGE', disks: DISKS, minimum_bytes: 20e9, refused: refused || null });
  F.out.push({ event: 'hello', protocol: 1 });
  step({ step: 'WELCOME', release_id: '9d2c3d2197b8', development: true, language: 'pt' });

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
    const right = () => F.disk && a.typed.trim().toUpperCase() === F.disk.confirmation_token.toUpperCase();
    if (F.at === 'WELCOME' || F.at === 'DRIVEN_ELSEWHERE') {
      if (a.action !== 'begin') return;
      if (scenario === 'elsewhere') return step({ step: 'DRIVEN_ELSEWHERE' });
      const blocked = scenario === 'blocked' ? { code: 'STORAGE_PROTECTION_UNVERIFIED', reason: 'swap_active' } : null;
      return step({ step: 'COMPATIBILITY', uefi: true, architecture_supported: true, memory_bytes: 16 * 1073741824, memory_required: 2 * 1073741824, storage_protected: !blocked, blocked });
    }
    if (F.at === 'COMPATIBILITY') { if (a.action === 'continue' && scenario !== 'blocked') storage(); return; }
    if (F.at === 'STORAGE') {
      if (a.action === 'rescan') return storage();
      if (a.action !== 'select_disk') return;
      const d = DISKS.find((x) => x.by_path === a.by_path);
      if (!d) return storage({ code: 'AMBIGUOUS_DISK' });
      if (d.protection !== 'ELIGIBLE' && d.protection !== 'AMBIGUOUS') return storage({ code: 'DISK_PROTECTED', protection: d.protection });
      F.disk = d;
      return step({ step: 'REVIEW', disk: d, confirmation_hint: d.serial || d.confirmation_token, confirmation_length: d.confirmation_token.length });
    }
    if (F.at === 'REVIEW') {
      if (a.action === 'back') { F.disk = null; return storage(); }
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
    if (cmd === 'installer_poll') { const events = F.out; F.out = []; return { state: 'OPEN', events }; }
    if (cmd === 'installer_send') { act(args.action); return true; }
    return null;
  };
})();
