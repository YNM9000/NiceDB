# Risolvere i problemi

## La connessione non si apre
1. Tasto destro sulla connessione › **Modifica…**.
2. **Prova**: sotto il form compare il messaggio del server o del driver.
3. Controlla in ordine:
   - **Host** e **Porta**: per ClickHouse la porta HTTP (8123, o 8443 con TLS), non la 9000;
   - **Utente** e **Password**: un campo password vuoto con «Salvata nel portachiavi» usa quella salvata;
   - **Database**: deve esistere ed essere accessibile all'utente;
   - rete e firewall, oppure il **Tunnel SSH** o il proxy, se il server non è raggiungibile direttamente;
   - le variabili `${NOME}` nei campi: devono avere un valore nello spazio di lavoro o nel sistema.

Se il messaggio è «L'istruzione di init 2 non è andata: …», la connessione funziona ma una delle **Query di init** (sezione **Sessione** del form) fallisce: correggila o toglila.

## Errori TLS
- **Verifica il certificato** fallisce con un certificato firmato da una CA interna: indica il file della CA in **Certificato CA (facoltativo)**.
- Con un certificato autofirmato, o per una prova veloce, scegli **Cifrato, senza verifica**: il traffico resta cifrato, ma niente garantisce di parlare con il server giusto.
- **Senza TLS** solo se il server non lo prevede.
- MongoDB e Redis attraverso un tunnel SSH o un proxy vedono il server come `127.0.0.1`: con la verifica attiva, il certificato deve valere anche per quel nome.
- Con **Usa un certificato client**, una chiave cifrata va in formato PKCS#8 (`openssl pkcs8 -topk8`); SQL Server accetta solo chiavi senza passphrase.

## Chiave SSH del server
**Chiave sconosciuta**: la prima volta che passi da un host SSH, **Prova** mostra «Chiave del server sconosciuta» con l'impronta della chiave.
1. Confronta l'impronta con quella che ti dà chi gestisce il server.
2. Se coincide, **Fidati e riprova**.

La chiave viene salvata nel file `known_hosts` della cartella dati di NiceDB (vedi sotto). NiceDB legge anche `~/.ssh/known_hosts`: gli host già noti al sistema non chiedono niente. Lo stesso vale per ogni host di **Passa prima da**.

**Chiave cambiata**: compare «La chiave di *host* è cambiata. Potrebbe essere un attacco: la connessione è bloccata finché non verifichi.».
1. Chiedi a chi gestisce il server se la chiave è stata cambiata davvero.
2. Solo se è così, togli la riga di quell'host dal file `known_hosts` che la contiene (quello di NiceDB o `~/.ssh/known_hosts`).
3. **Prova** di nuovo e fidati della chiave nuova.

## Connessione persa
Se la rete cade o il server si riavvia, la barra di stato dice «connessione persa», poi «riconnessione…». NiceDB riapre la sessione con lo stesso profilo, tunnel e password e riesegue le query di init; un avviso conferma il ripristino.
- Se il server non risponde, i tentativi ripartono da soli dopo 5 s, 10 s, 20 s, fino a un minuto. **Riconnetti** li forza subito.
- Una query interrotta non viene mai rilanciata da sola. Per una lettura l'avviso dice «Era una lettura: puoi rilanciarla senza rischi.» e offre **Rilancia da qui**; per una scrittura dice che potrebbe essere stata eseguita e offre **Rilancia comunque**: controlla i dati prima.
- Una transazione aperta va persa: l'avviso lo dice.
- Variabili impostate a mano nell'editor e tabelle temporanee non sopravvivono.
- Una connessione di produzione riparte in sola lettura.

Per non riconnettere in automatico togli **Riconnetti da sola se la connessione cade** nella sezione **Sessione** del form. Un server che smette di rispondere senza chiudere la connessione si scopre prima con il **Keep-alive** acceso. Le opzioni sono descritte in [Connessioni e profili](connections).

## Password e portachiavi
NiceDB non salva le password nei suoi file: le tiene nel portachiavi del sistema, con il nome di servizio `app.nicedb.desktop`.
- **Linux**: serve un servizio Secret Service attivo e sbloccato (GNOME Portachiavi, KWallet o simili). Le password vanno nel portachiavi predefinito; se manca, NiceDB usa quello `login`. Se non c'è nessuno dei due, il salvataggio dice «Manca un portachiavi predefinito»: creane uno con Seahorse (GNOME Keyring) o KWallet, rendilo predefinito e salva di nuovo.
- **macOS**: il Portachiavi di sistema.
- **Windows**: Gestione credenziali.

Un errore che inizia con `keychain:` viene dal portachiavi: sbloccalo o avvia il servizio, poi riprova. Se una password non si trova più, riscrivila nel form e **Salva**. **Togli la password salvata** la cancella dal portachiavi.

## La produzione non scrive
| Messaggio | Cosa fare |
|---|---|
| «Produzione in sola lettura: sblocca la scrittura per eseguire.» | sblocca con l'interruttore sotto le schede |
| «Serve la conferma con il nome della connessione.» | conferma nella finestra che compare |
| «Il nome scritto non corrisponde alla connessione.» | scrivi il nome esatto della connessione, maiuscole comprese |
| «tempo scaduto: si chiude a fine statement» | lo sblocco a tempo è finito: sblocca di nuovo |
| «Connessione di produzione: qui i dati di prova non si scrivono.» | Dati di prova non scrive mai in produzione |

Dopo una riconnessione lo sblocco va rifatto. Durata e conferme si scelgono in **Impostazioni › Sicurezza**: vedi [Sicurezza in produzione](production).

## Query fermate o tagliate
- «Interrotta dopo 60 s: limite di tempo dell'ambiente.»: la query ha superato il **Limite di tempo** del suo ambiente in **Impostazioni › Sicurezza**.
- «Limite della connessione: mostrate le prime 1000 righe»: **Carica tutto** rilegge senza il limite della connessione.
- Risultato segnato come troncato: ha raggiunto il **Tetto di righe per query** di **Impostazioni › Risultati**.
- «La connessione sta già eseguendo uno statement.»: aspetta o premi [[key:cancel]], oppure apri un'altra scheda con [[key:new_tab]].
- «C'è già una transazione aperta: confermala o annullala prima.»: vedi [Transazioni e prova a secco](transactions).

## Schema lento o incompleto
- Lo schema si legge una volta, all'apertura della connessione o del database, e con **Aggiorna schema**. Dopo modifiche fatte fuori da NiceDB, aggiorna.
- Con migliaia di tabelle la palette e il completamento sono pronti poco dopo l'arrivo dello schema; nel frattempo la palette dice «Lo schema si sta caricando…».
- Su SQL Server la prima lettura dopo l'avvio del server può impiegare qualche secondo.
- Lo schema ER di migliaia di tabelle compare dopo: filtra per schema o per nome.
- Le tabelle di un database mai aperto non si trovano: apri il database dalla tendina.
- MongoDB e ClickHouse leggono lo schema del solo database scelto.

## Dove sono i dati di NiceDB
Tutto quello che NiceDB salva sta in una cartella dell'utente:

| Sistema | Cartella |
|---|---|
| Linux | `~/.local/share/app.nicedb.desktop` (o `$XDG_DATA_HOME/app.nicedb.desktop`) |
| macOS | `~/Library/Application Support/app.nicedb.desktop` |
| Windows | `%APPDATA%\app.nicedb.desktop` |

- `nicedb.sqlite`: connessioni, cartelle, spazi di lavoro, impostazioni, cronologia, registro delle scritture, script, snippet, note, segni della palette e profili dei dati di prova.
- `known_hosts`: le chiavi SSH accettate con **Fidati e riprova**.

Le password non stanno qui ma nel portachiavi. Per una copia di sicurezza delle connessioni conviene un pacchetto `.nicedb` ([Importare ed esportare](import-export)). Per ripartire da zero, chiudi NiceDB e sposta la cartella altrove: al riavvio compare la configurazione iniziale.
