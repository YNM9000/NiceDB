# Connessioni e profili

## Creare una connessione
La finestra ha due parti: a sinistra la lista dei motori, a destra i campi del motore scelto, con **Prova**, **Annulla** e **Salva** in fondo.

1. Premi il **+** accanto a **Connessioni**, oppure, su una cartella, tasto destro › **Nuova connessione qui…**.
2. Scegli il motore. Il cursore è già in **Cerca un motore**: scrivi parte del nome e la lista si restringe, e il primo motore che resta diventa la scelta. [[↑]] [[↓]] cambiano motore, [[Pag↑]] [[Pag↓]] saltano di otto, [[Enter]] porta al **Nome**, [[Esc]] svuota la ricerca.
3. Scrivi il **Nome** e scegli la **Cartella**.
4. Compila i campi di collegamento (tabella sotto).
5. Scegli l'**Ambiente**: **locale**, **sviluppo**, **staging** o **produzione**.
6. Premi **Prova**: il risultato compare accanto al pulsante. Poi **Salva**.

I campi obbligatori mancanti si colorano al primo **Prova** o **Salva**.

La lista divide i motori per famiglia: relazionali, documenti, chiave-valore, colonne larghe, ricerca, analitici, aziendali. I prodotti che parlano il protocollo di un altro motore stanno rientrati sotto di lui (TimescaleDB, YugabyteDB, Neon e Supabase sotto PostgreSQL; TiDB e PlanetScale sotto MySQL). La ricerca trova anche per sigla o per prodotto compatibile (`mssql`, `scylla`, `opensearch`, `valkey`), per famiglia e per luogo: `file` mostra i motori su file, `cloud` quelli ospitati.

In alto a destra la finestra ripete il motore scelto, con la famiglia, dove stanno i dati e, per i prodotti compatibili, il protocollo usato.

**Modifica** apre la connessione direttamente sul suo motore, con la lista chiusa; il pulsante a sinistra del marchio la riapre. Dopo il salvataggio il motore non si cambia più, salvo passare tra i prodotti dello stesso protocollo (PostgreSQL e TimescaleDB, per esempio): gli altri motori nella lista restano spenti. Per un altro motore si crea una connessione nuova.

| Motore | Campi | Porta predefinita |
|---|---|---|
| PostgreSQL, CockroachDB | host, porta, database, utente, password | 5432, 26257 |
| MySQL / MariaDB | host, porta, database, utente, password | 3306 |
| TimescaleDB, YugabyteDB, Neon, Supabase | come PostgreSQL | 5432; YugabyteDB 5433 |
| TiDB, PlanetScale | come MySQL | 4000, 3306 |
| SQL Server | host, porta, database, utente, password | 1433 |
| Oracle | host, porta, **Servizio** (o SID), utente, password, **Privilegio di accesso**; oppure **Stringa di connessione** (Easy Connect o descrittore) | 1521 |
| SQLite | **File del database**, **Apri in sola lettura** | |
| DuckDB | **File del database** (vuoto per un database in memoria), **Apri in sola lettura** | |
| libSQL / Turso | **Dove**: **File locale** (come SQLite) o **Remoto** con **URL** (`libsql://`, `https://`, `wss://`) e **Token di accesso** | |
| ClickHouse | host, porta, database, utente, password | 8123, 8443 con TLS |
| MongoDB | host, porta, database, utente, password, **Database di autenticazione**, **Opzioni della connessione**, **Record DNS SRV (mongodb+srv)** | 27017 |
| Redis / Valkey | host, porta, **Database (numero)**, utente facoltativo, password | 6379 |

- **Scegli…** accanto al file apre la finestra del sistema.
- Per PostgreSQL, CockroachDB, MySQL e i [motori compatibili](compatible), **URL di connessione** compila host, porta, database, utente e password da un URL incollato (`postgresql://…`, `cockroachdb://…`, `mysql://…`); `sslmode` decide il TLS. Il motore scelto resta quello del form quando l'URL gli va bene: un `postgresql://` incollato con CockroachDB o TimescaleDB scelto non torna a PostgreSQL.
- Oracle si raggiunge con host, porta e servizio, oppure con una stringa Easy Connect o un descrittore; l'URL è `oracle://utente@host:1521/servizio`. Non serve Instant Client. Vedi [Oracle](oracle).
- ClickHouse passa dall'interfaccia HTTP; la porta nativa 9000 non serve. Vedi [ClickHouse](clickhouse).
- Con **Record DNS SRV** l'host è il nome del cluster: server e porta arrivano dal DNS, e tunnel SSH e proxy non si usano. Vedi [MongoDB](mongodb) e [Redis e Valkey](redis).

## Password e portachiavi
Password, token, passphrase delle chiavi e password di proxy e salti SSH vanno nel portachiavi del sistema, mai nel profilo. In modifica il campo dice **Salvata nel portachiavi**: lasciarlo vuoto tiene la password. **Togli la password salvata** la cancella.

## TLS
| Scelta | Effetto |
|---|---|
| **Verifica il certificato** | cifrato, con controllo del certificato del server; **Certificato CA (facoltativo)** per una CA propria |
| **Cifrato, senza verifica** | cifrato, ma nulla prova che il server sia quello giusto |
| **Senza TLS** | in chiaro |

Per `localhost`, `127.*` e `::1` la scelta parte da **Senza TLS**, per gli altri host da **Verifica il certificato**. Neon, Supabase e PlanetScale accettano solo connessioni cifrate: **Senza TLS** non c'è.

**Usa un certificato client** (PostgreSQL, CockroachDB, MySQL/MariaDB, SQL Server) chiede **Certificato (PEM)**, **Chiave privata (PEM)** e **Passphrase della chiave**. Una chiave cifrata va in formato PKCS#8:

```
openssl pkcs8 -topk8 -v2 aes-256-cbc -in chiave.pem -out chiave-pkcs8.pem
```

## Tunnel SSH
1. Spunta **Tunnel SSH**.
2. Scrivi **Host SSH**, porta e **Utente SSH**.
3. Scegli l'**Accesso**: **Agent** (le chiavi caricate in ssh-agent), **Chiave** (**File della chiave** e **Passphrase della chiave**) o **Password**.
4. Per attraversare altri host prima, usa **Passa prima da** › **Aggiungi un salto**, fino a 8. Le frecce cambiano l'ordine, la × toglie il salto.

Alla prima connessione compare **Chiave del server sconosciuta** con l'impronta: confrontala con quella del server, poi premi **Fidati e riprova**. Con i salti la domanda arriva per un host alla volta. Una chiave cambiata blocca la connessione.

## Proxy
**Passa da un proxy** apre **Tipo di proxy** (**SOCKS5** o **HTTP CONNECT**), **Host del proxy**, porta, **Utente del proxy** e **Password del proxy**. Il proxy porta il primo tratto: fino al database o fino al primo host SSH.

## Sessione
La sezione **Sessione**, in fondo al form, si apre con un clic; chiusa, riassume il contenuto in una riga.

| Campo | Effetto |
|---|---|
| **Query di init** | istruzioni eseguite a ogni apertura della sessione, dopo una riconnessione, al cambio di database e nelle connessioni di servizio di strumenti e import. Se una fallisce, la sessione non si apre e l'errore dice quale |
| **Keep-alive** | un `SELECT 1` sulla sessione ferma ogni 30 s, 1, 2, 5 o 10 minuti; mai mentre gira una query. Solo per i motori in rete |
| **Chiudi se ferma da** | dopo 15 min … 8 h senza query la sessione si chiude; la scheda resta e alla prima query si riapre |
| **Limite di righe** | per le `SELECT` senza `LIMIT`, `TOP` o `FETCH`; vuoto vale il tetto di **Impostazioni › Risultati** |
| **Riconnetti da sola se la connessione cade** | riapre la sessione con lo stesso profilo, tunnel e password, poi rilancia le query di init |

Esempi di init: `SET search_path TO vendite, public;` su PostgreSQL, `SET NAMES utf8mb4;` su MySQL, `PRAGMA foreign_keys = ON;` su SQLite.

Quando la connessione cade, la barra di stato dice **connessione persa** o **riconnessione…**. Una query interrotta non viene mai rilanciata da sola: la scheda propone **Rilancia da qui**, o **Rilancia comunque** se era una scrittura. **Riconnetti** forza un nuovo tentativo. Una transazione aperta va persa con la connessione.

## Variabili nei campi
In host, porta, database, utente, file, URL, certificati, campi SSH e proxy e query di init si può scrivere `${NOME}`, anche dentro un testo: `db.${AMBIENTE}.bassi.lan`.
- Il valore viene dalle variabili dello spazio di lavoro attivo, poi dall'ambiente del sistema. Vedi [Spazi di lavoro e cartelle](workspaces).
- `${PORTA:-5432}` usa 5432 se la variabile manca; `$${` scrive un `${` letterale.
- Sotto il campo compare il valore finale («Diventa …») o il problema.
- Nomi che sembrano segreti (PASSWORD, TOKEN, SECRET…) sono rifiutati: le password restano nel portachiavi.

## Modelli
Un modello è un profilo che non si apre e detta i valori a più connessioni, per esempio staging e produzione dello stesso cliente.
1. Nel campo **Modello** scegli **È un modello per altre connessioni** e salva.
2. Sul modello, tasto destro › **Nuova connessione dal modello**.
3. Cambia solo quello che serve: ogni campo cambiato diventa **proprio**; ↺ **Riprendi dal modello** lo riallinea.

Le modifiche al modello arrivano subito a chi lo segue, tranne nei campi propri. Nome, cartella, ambiente e password restano di ogni connessione. Nell'albero il modello ha il nome in corsivo e l'etichetta **modello**; il clic lo apre in modifica.

## Gestire le connessioni
Clic su una connessione la apre e mostra l'albero degli oggetti; se è già aperta, porta alla sua scheda. Accanto al nome: un segno mentre si apre, uno di errore se l'apertura fallisce (il motivo è nel suggerimento), e i pulsanti per disconnettere e modificare.

Il tasto destro offre: **Apri** o **Vai alla connessione**, **Nuova scheda**, **Strumenti** (vedi [Strumenti](tools)), **Modifica…**, **Duplica…**, **Copia come URL** (senza password), **Sposta in**, **Spazi di lavoro**, **Esporta…**, **Disconnetti** ed **Elimina…**.

Eliminare chiede conferma. Per 30 secondi [[Ctrl+Z]] riporta la connessione intera, password comprese.

## Produzione
Con l'ambiente **produzione** la connessione è segnata ovunque, nella barra del titolo e sulle schede, e parte in sola lettura. Sblocco e conferme sono in [Sicurezza in produzione](production).

## Limiti
- SQL Server accetta un certificato client solo con una chiave senza passphrase.
- Proxy e salti SSH valgono per i motori in rete; libSQL remoto non passa dal proxy.
- Una variabile cambiata vale dalla prossima apertura o riconnessione, non per le sessioni già aperte.
- Variabili di sessione impostate a mano nell'editor e tabelle temporanee non sopravvivono a una riconnessione.
- Una sessione di produzione ripristinata riparte in sola lettura.
