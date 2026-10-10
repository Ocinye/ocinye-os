/* Ocinye OS Installer · textos do Code (pt canónico · en · fr).
   Acrescentam-se aos do Design (strings.js) e nunca os substituem. Existem por
   duas razões, registadas em docs/ui/CODE_FEEDBACK.md:
   - a decisão de produto posterior ao pacote (servidor = Ubuntu Server 24.04
     LTS minimal, só), que torna falsos os textos que falam de Debian ou de
     «qualquer Linux com Docker»;
   - o produto mostra dados reais: onde a referência tinha um exemplo fixo
     («503 em docker-ce», «migração 0041»), aqui há um texto com parâmetros. */
(function () {
  'use strict';
  const X = {
    'x.tlsWeak': [
      'O certificado ou a cadeia estão assinados com SHA-1 ou MD5, a chave RSA tem menos de 2048 bits, ou a chave EC não nomeia a curva: o servidor ou os browsers recusam-no. Emita-o de novo com SHA-256 e uma curva nomeada.',
      'The certificate or its chain is signed with SHA-1 or MD5, the RSA key is shorter than 2048 bits, or the EC key does not name its curve: the server or browsers refuse it. Reissue it with SHA-256 and a named curve.',
      'Le certificat ou sa chaîne est signé avec SHA-1 ou MD5, la clé RSA fait moins de 2048 bits, ou la clé EC ne nomme pas sa courbe : le serveur ou les navigateurs le refusent. Émettez-le de nouveau avec SHA-256 et une courbe nommée.'],
    'x.welcomeNeed2': [
      'Um servidor Ubuntu Server 24.04 LTS de 64 bits (instalação mínima, sem ambiente gráfico), com acesso SSH e sudo. O instalador instala o Docker quando falta.',
      'A 64-bit Ubuntu Server 24.04 LTS server (minimal installation, no graphical environment) with SSH and sudo access. The installer installs Docker when it is missing.',
      'Un serveur Ubuntu Server 24.04 LTS 64 bits (installation minimale, sans environnement graphique) avec accès SSH et sudo. L’installateur installe Docker s’il manque.'],
    'x.preDockerInstall': [
      'Não instalado · o plano vai instalar Docker Engine e o plugin compose a partir do repositório oficial do Docker para Ubuntu 24.04. Nada é instalado nesta verificação.',
      'Not installed · the plan will install Docker Engine and the compose plugin from Docker’s official repository for Ubuntu 24.04. Nothing is installed by this check.',
      'Non installé · le plan installera Docker Engine et le plugin compose depuis le dépôt officiel de Docker pour Ubuntu 24.04. Rien n’est installé par cette vérification.'],
    'x.tlsProvided': ['fornecido · válido até {d}', 'provided · valid until {d}', 'fourni · valable jusqu’au {d}'],
    'x.doneNext1': [
      'Entre em {u} com a identidade privilegiada; defina a palavra-passe e o segundo factor.',
      'Sign in at {u} with the privileged identity; set the password and second factor.',
      'Connectez-vous sur {u} avec l’identité privilégiée ; définissez le mot de passe et le second facteur.'],
    'x.verLoginD': ['{u}/ → 303 → /boot → 200 → /login → 200', '{u}/ → 303 → /boot → 200 → /login → 200', '{u}/ → 303 → /boot → 200 → /login → 200'],
    'x.osD': ['{v} · núcleo {k}', '{v} · kernel {k}', '{v} · noyau {k}'],
    'x.distroBlocked': [
      'O Ocinye OS v1 instala-se só em Ubuntu Server 24.04 LTS (instalação mínima, sem ambiente gráfico). Este servidor tem {v}.',
      'Ocinye OS v1 installs only on Ubuntu Server 24.04 LTS (minimal installation, no graphical environment). This server runs {v}.',
      'Ocinye OS v1 s’installe uniquement sur Ubuntu Server 24.04 LTS (installation minimale, sans environnement graphique). Ce serveur exécute {v}.'],
    'x.distroGui': [
      'Este servidor tem um ambiente gráfico instalado. O Ocinye OS instala-se num servidor sem interface gráfica.',
      'This server has a graphical environment installed. Ocinye OS installs on a server without a graphical interface.',
      'Ce serveur a un environnement graphique installé. Ocinye OS s’installe sur un serveur sans interface graphique.'],
    'x.archBlocked': ['Servidor {a}; o release é {b}.', 'Server {a}; the release is {b}.', 'Serveur {a} ; la version est {b}.'],
    'x.timeBlocked': ['O relógio do servidor está {s} s desfasado deste computador.', 'The server clock is {s} s away from this computer’s.', 'L’horloge du serveur s’écarte de {s} s de celle de cet ordinateur.'],
    'x.cpuBlocked': ['{n} vCPU; o mínimo é 2.', '{n} vCPU; the minimum is 2.', '{n} vCPU ; le minimum est 2.'],
    'x.ramBlock': ['{v} de memória; o mínimo medido é 3,5 GB.', '{v} of memory; the measured minimum is 3.5 GB.', '{v} de mémoire ; le minimum mesuré est 3,5 Go.'],
    'x.hwPartial': ['/proc, /sys ou o barramento PCI não são legíveis: a descoberta de hardware fica incompleta.', '/proc, /sys or the PCI bus are not readable: hardware discovery will be incomplete.', '/proc, /sys ou le bus PCI ne sont pas lisibles : la découverte du matériel sera incomplète.'],
    'x.dockerOld': [
      'Docker {v}: é preciso o Docker Engine 24 ou mais recente, com o plugin compose v2. O instalador não actualiza um runtime que não instalou.',
      'Docker {v}: Docker Engine 24 or later with the compose v2 plugin is required. The Installer does not upgrade a runtime it did not install.',
      'Docker {v} : Docker Engine 24 ou plus récent, avec le plugin compose v2, est requis. L’installateur ne met pas à jour un runtime qu’il n’a pas installé.'],
    'x.dockerUnsupported': ['Sem runtime de contentores, e este sistema não é o suportado: o instalador só instala o Docker em Ubuntu Server 24.04 LTS.', 'No container runtime, and this system is not the supported one: the Installer only installs Docker on Ubuntu Server 24.04 LTS.', 'Aucun runtime de conteneurs, et ce système n’est pas celui pris en charge : l’installateur n’installe Docker que sur Ubuntu Server 24.04 LTS.'],
    'x.pkgInstall': ['Do arquivo do Ubuntu: {p} (será instalado).', 'From the Ubuntu archive: {p} (will be installed).', 'Depuis l’archive Ubuntu : {p} (sera installé).'],
    'x.pkgMissing': ['Falta {p}, que um servidor Ubuntu tem sempre.', '{p} is missing, which an Ubuntu server always has.', '{p} manque, alors qu’un serveur Ubuntu l’a toujours.'],
    'x.proxyD': ['{u} activo, sem usar as portas 80/443.', '{u} active, not using ports 80/443.', '{u} actif, sans utiliser les ports 80/443.'],
    'x.registryBlocked': ['Sem acesso HTTPS de saída a registry-1.docker.io e download.docker.com.', 'No outbound HTTPS to registry-1.docker.io and download.docker.com.', 'Pas d’accès HTTPS sortant vers registry-1.docker.io et download.docker.com.'],
    'x.systemdNone': ['Sem systemd: a Instância não volta sozinha depois de um reboot.', 'No systemd: the Instance will not come back by itself after a reboot.', 'Pas de systemd : l’Instance ne redémarrera pas seule après un reboot.'],
    'x.srvdirBlocked': ['/srv/ocinye existe, não está vazio, e não é de uma instalação deste instalador.', '/srv/ocinye exists, is not empty, and is not from an installation by this Installer.', '/srv/ocinye existe, n’est pas vide et ne provient pas d’une installation de cet installateur.'],
    'x.fwConflict': ['O ufw recusa explicitamente a porta {p}. Remova essa regra fora do instalador.', 'ufw explicitly denies port {p}. Remove that rule outside the Installer.', 'ufw refuse explicitement le port {p}. Supprimez cette règle hors de l’installateur.'],
    'x.fwApply': ['ufw activo: {r} com o comentário «ocinye» (será aplicado).', 'ufw active: {r} with the comment “ocinye” (will be applied).', 'ufw actif : {r} avec le commentaire « ocinye » (sera appliqué).'],
    'x.fwPresent': ['ufw activo e já permite 80/tcp e 443/tcp.', 'ufw active and already allows 80/tcp and 443/tcp.', 'ufw actif et autorise déjà 80/tcp et 443/tcp.'],
    'x.journalFound': ['Há uma instalação incompleta deste instalador.', 'There is an incomplete installation by this Installer.', 'Il y a une installation incomplète de cet installateur.'],
    'x.chPrereq': [
      'Instala o Docker Engine e o plugin compose ({p}) do repositório oficial do Docker para Ubuntu Server 24.04 LTS, com a chave do repositório verificada pela impressão digital do manifesto ({f}). Não havia runtime de contentores neste servidor.',
      'Installs Docker Engine and the compose plugin ({p}) from Docker’s official repository for Ubuntu Server 24.04 LTS, the repository key verified against the manifest fingerprint ({f}). This server had no container runtime.',
      'Installe Docker Engine et le plugin compose ({p}) depuis le dépôt officiel de Docker pour Ubuntu Server 24.04 LTS, la clé du dépôt vérifiée par l’empreinte du manifeste ({f}). Ce serveur n’avait aucun runtime de conteneurs.'],
    'x.chPkg': ['Instala do arquivo do Ubuntu: {p}.', 'Installs from the Ubuntu archive: {p}.', 'Installe depuis l’archive Ubuntu : {p}.'],
    'x.chFw': [
      'Acrescenta ao ufw: {r}, com o comentário «ocinye». Não remove nem altera outras regras; a porta SSH fica como está.',
      'Adds to ufw: {r}, with the comment “ocinye”. Does not remove or change other rules; the SSH port stays as it is.',
      'Ajoute à ufw : {r}, avec le commentaire « ocinye ». Ne supprime ni ne modifie d’autres règles ; le port SSH reste tel quel.'],
    'x.failR': ['A fase {p} terminou com o código {c}.', 'Phase {p} ended with code {c}.', 'La phase {p} s’est terminée avec le code {c}.'],
    'x.failRetry': ['Pode tentar novamente: a fase repete-se sem refazer o que já ficou feito.', 'You can try again: the phase repeats without redoing what is already done.', 'Vous pouvez réessayer : la phase se répète sans refaire ce qui est déjà fait.'],
    'x.failNoRetry': ['Não se repete sem perceber a causa: guarde o relatório. Enquanto não houver Instância, pode remover a instalação incompleta.', 'It is not repeated without understanding the cause: save the report. While there is no Instance, you can remove the incomplete installation.', 'Elle n’est pas répétée sans comprendre la cause : enregistrez le rapport. Tant qu’il n’y a pas d’Instance, vous pouvez supprimer l’installation incomplète.'],
    'x.repoKeyT': ['P04 · a chave do repositório do Docker não confere', 'P04 · the Docker repository key does not match', 'P04 · la clé du dépôt Docker ne correspond pas'],
    'x.repoKeyR': ['A chave descarregada não tem a impressão digital que o manifesto do release fixa. Nada foi instalado a partir desse repositório.', 'The downloaded key does not have the fingerprint the release manifest pins. Nothing was installed from that repository.', 'La clé téléchargée n’a pas l’empreinte que le manifeste de la version fixe. Rien n’a été installé depuis ce dépôt.'],
    'x.upload': ['{a} de {b} enviados', '{a} of {b} sent', '{a} sur {b} envoyés'],
    'x.op.verify_bootstrap': ['ocinye-bootstrap conferido (SHA-256)', 'ocinye-bootstrap verified (SHA-256)', 'ocinye-bootstrap vérifié (SHA-256)'],
    'x.op.verify_artifact': ['{p} · soma conferida no servidor ({d} de {t})', '{p} · sum verified on the server ({d} of {t})', '{p} · somme vérifiée sur le serveur ({d} sur {t})'],
    'x.op.recheck_preflight': ['a confirmar a verificação prévia', 'confirming the preflight', 'confirmation de la vérification préalable'],
    'x.op.apt_install': ['apt · {p}', 'apt · {p}', 'apt · {p}'],
    'x.op.verify_docker_key': ['chave do repositório do Docker: impressão digital conferida', 'Docker repository key: fingerprint verified', 'clé du dépôt Docker : empreinte vérifiée'],
    'x.op.enable_docker': ['serviço docker activo', 'docker service active', 'service docker actif'],
    'x.op.extract_release': ['a extrair o release para /srv/ocinye', 'extracting the release to /srv/ocinye', 'extraction de la version vers /srv/ocinye'],
    'x.op.write_configuration': ['a gerar a configuração e os segredos no servidor', 'generating the configuration and secrets on the server', 'génération de la configuration et des secrets sur le serveur'],
    'x.op.write_proxy': ['nginx para todos os pontos de acesso', 'nginx for every access endpoint', 'nginx pour tous les points d’accès'],
    'x.op.install_tls': ['certificado instalado', 'certificate installed', 'certificat installé'],
    'x.op.firewall_allow': ['ufw allow {p}/tcp · comentário ocinye', 'ufw allow {p}/tcp · comment ocinye', 'ufw allow {p}/tcp · commentaire ocinye'],
    'x.op.load_image': ['a carregar ocinye-{s}', 'loading ocinye-{s}', 'chargement de ocinye-{s}'],
    'x.op.start_persistence': ['PostgreSQL, Redis e armazenamento', 'PostgreSQL, Redis and storage', 'PostgreSQL, Redis et stockage'],
    'x.op.create_instance': ['migrações, Instância e primeiro administrador', 'migrations, Instance and first administrator', 'migrations, Instance et premier administrateur'],
    'x.op.seed_endpoint': ['ponto {h}', 'endpoint {h}', 'point {h}'],
    'x.op.wait_healthy': ['à espera de {s}', 'waiting for {s}', 'en attente de {s}'],
    'x.op.enable_systemd': ['ocinye.service activo', 'ocinye.service enabled', 'ocinye.service activé'],
    'x.op.remove_bootstrap': ['a remover o executor temporário', 'removing the temporary executor', 'suppression de l’exécuteur temporaire'],
    'x.alert.transportT': ['A ligação ao servidor falhou', 'The connection to the server failed', 'La connexion au serveur a échoué'],
    'x.alert.bootstrapT': ['O executor temporário recusou continuar', 'The temporary executor refused to continue', 'L’exécuteur temporaire a refusé de continuer'],
    'x.alert.bootstrapB': ['Código {c}. Nada foi alterado no servidor.', 'Code {c}. Nothing was changed on the server.', 'Code {c}. Rien n’a été modifié sur le serveur.'],
    'x.alert.configT': ['A configuração tem um erro', 'The configuration has an error', 'La configuration comporte une erreur'],
    'x.alert.planT': ['Não há plano guardado para retomar', 'There is no saved plan to resume', 'Aucun plan enregistré à reprendre'],
    'x.alert.planB': ['A instalação incompleta foi iniciada noutro computador ou o plano não foi guardado. Pode removê-la enquanto não houver Instância.', 'The incomplete installation was started on another computer or the plan was not saved. You can remove it while there is no Instance.', 'L’installation incomplète a été lancée sur un autre ordinateur ou le plan n’a pas été enregistré. Vous pouvez la supprimer tant qu’il n’y a pas d’Instance.'],
    'x.field.port': ['Uma porta entre 1 e 65535.', 'A port between 1 and 65535.', 'Un port entre 1 et 65535.'],
    'x.field.user': ['Um nome de utilizador: minúsculas, algarismos, «_» e «-».', 'A user name: lowercase letters, digits, “_” and “-”.', 'Un nom d’utilisateur : minuscules, chiffres, « _ » et « - ».'],
    'x.field.key': ['Escolha o ficheiro da chave privada.', 'Choose the private key file.', 'Choisissez le fichier de la clé privée.'],
    'x.field.pw': ['Escreva a palavra-passe.', 'Type the password.', 'Saisissez le mot de passe.'],
    'x.field.text': ['Obrigatório, até 120 caracteres, sem caracteres de controlo.', 'Required, up to 120 characters, no control characters.', 'Obligatoire, jusqu’à 120 caractères, sans caractères de contrôle.'],
    'x.field.invalid': ['Valor inválido.', 'Invalid value.', 'Valeur invalide.'],
    'x.field.endpoint': ['Um nome de anfitrião, e uma Distribuição activada para os pontos fixos.', 'A host name, and an enabled Distribution for bound endpoints.', 'Un nom d’hôte, et une Distribution activée pour les points liés.'],
    'x.sudoRefused': ['A palavra-passe foi recusada pelo sudo.', 'sudo refused the password.', 'sudo a refusé le mot de passe.'],
    'x.forget': ['Esquecer este servidor', 'Forget this server', 'Oublier ce serveur'],
    'x.forgetT': ['Esquecer a identidade guardada?', 'Forget the stored identity?', 'Oublier l’identité enregistrée ?'],
    'x.forgetB': ['A impressão digital guardada é apagada. A próxima ligação volta a ser um primeiro contacto, com a impressão nova à vista para confiar ou não. Faça-o só se sabe porque é que a chave do servidor mudou.', 'The stored fingerprint is deleted. The next connection is a first contact again, with the new fingerprint shown for you to trust or not. Only do this if you know why the server key changed.', 'L’empreinte enregistrée est supprimée. La prochaine connexion redevient un premier contact, avec la nouvelle empreinte affichée pour décider de lui faire confiance. Ne le faites que si vous savez pourquoi la clé du serveur a changé.'],
    'x.forgetConfirm': ['Esquecer e voltar a verificar', 'Forget and verify again', 'Oublier et vérifier à nouveau'],
    'x.lang': ['Língua', 'Language', 'Langue'],
    'x.noneGpu': ['Nenhuma GPU detectada', 'No GPU detected', 'Aucun GPU détecté'],
    'x.verifyFailedB': ['Um item obrigatório do servidor não passou. A instalação não está concluída e o Ocinye OS não está operacional.', 'A mandatory server item did not pass. The installation is not complete and Ocinye OS is not operational.', 'Un élément obligatoire côté serveur n’a pas réussi. L’installation n’est pas terminée et Ocinye OS n’est pas opérationnel.'],
    'x.credLost': ['A credencial temporária não pôde ser recuperada depois do corte. Siga o procedimento de recuperação de acesso administrativo.', 'The temporary credential could not be recovered after the interruption. Follow the administrative access recovery procedure.', 'L’identifiant temporaire n’a pas pu être récupéré après l’interruption. Suivez la procédure de récupération de l’accès administratif.'],
    'x.unreachable': ['Não foi possível ligar a {h}:{p}.', 'Could not connect to {h}:{p}.', 'Impossible de se connecter à {h}:{p}.'],
  };
  for (const k in X) window.OI_T[k] = X[k];
})();
