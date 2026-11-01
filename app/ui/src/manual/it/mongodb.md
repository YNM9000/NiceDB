# MongoDB

## Connettersi a MongoDB
1. **Nuova connessione**, poi in **Motore** scegli MongoDB.
2. Compila **Host**, **Porta** (27017 di solito), **Utente** e **Password**. Utente vuoto vuol dire server senza autenticazione.
3. Se serve, compila i campi propri di MongoDB:
   - **Record DNS SRV (mongodb+srv)**: l'host è il nome del cluster (`cluster0.ab12c.mongodb.net`); server e porta arrivano dal DNS. Con l'SRV il tunnel SSH e il proxy non si usano.
   - **Database di autenticazione**: dove è definito l'utente; vuoto vale `admin`.
   - **Opzioni della connessione**: la parte dopo `?` di un URI, per esempio `replicaSet=rs0&readPreference=secondaryPreferred`.
4. In **TLS** scegli **Verifica il certificato** (con un **Certificato CA (facoltativo)**), **Cifrato, senza verifica** o **Senza TLS**.
5. **Prova**, poi **Salva**.

La password sta nel portachiavi del sistema. Con il tunnel SSH o il proxy NiceDB si collega al solo server indicato, anche se fa parte di un replica set. Le variabili `${NOME}` valgono in tutti i campi.

## Sfogliare database e collezioni
Sotto la connessione ci sono i database; aprendone uno, le **collezioni** e le **viste**. Ogni collezione mostra il numero stimato di documenti, i campi e gli indici.

- I campi sono ricavati da un campione di 100 documenti. Accanto al campo c'è il tipo; con tipi misti la quota di ciascuno, per esempio `decimal 98% · string 2%`.
- I campi annidati si scrivono con il punto: `indirizzo.citta`, `righe.articolo`.
- Un clic su una collezione apre una scheda con un `find` sui primi 100 documenti e la esegue.

Dal tasto destro su una collezione: **Apri i documenti**, **Nuova query qui**, **Mostra il comando di creazione** (opzioni e indici, pronti da rieseguire), **Copia nome**, **Copia il comando find**.

## Scrivere comandi
L'editor accetta i comandi di database di MongoDB in JSON, gli stessi di `db.runCommand`:

```json
// clienti di Bergamo, solo nome e città
{ "find": "clienti",
  "filter": { "indirizzo.provincia": "BG" },
  "projection": { "ragione_sociale": 1, "indirizzo.citta": 1 },
  "sort": { "ragione_sociale": 1 },
  "limit": 50 }

{ "aggregate": "ordini", "pipeline": [
  { "$unwind": "$righe" },
  { "$group": { "_id": "$stato", "righe": { "$sum": 1 } } } ] }
```

- [[key:run_statement]] esegue il comando sotto il cursore, [[key:run_script]] li esegue tutti in ordine. I commenti `//` sono ammessi.
- Valori speciali in JSON esteso: `{ "$oid": "…" }`, `{ "$date": "2026-10-29T10:00:00Z" }`, `{ "$numberDecimal": "29.98" }`.
- In un `aggregate` il `cursor` si può omettere.
- Gli errori di sintassi sono sottolineati mentre scrivi.
- L'autocompletamento propone comandi, chiavi, operatori (`$match`, `$gt`…), le collezioni dopo `"find":` e i campi della collezione; dopo `"$` i riferimenti ai campi (`$cliente._id`).
- [[key:format]] riscrive ogni comando su più righe; un comando con commenti resta com'è.
- [[key:cancel]] ferma il cursore e chiede al server di fermare l'operazione.

## Leggere i risultati
I documenti arrivano a blocchi fino al limite di righe della connessione o al tetto di **Impostazioni › Risultati**. Sopra i risultati scegli la vista:
- **Documenti**: a sinistra l'elenco, a destra il documento scelto come albero, con il tipo BSON di ogni valore (`objectId`, `date`, `decimal`…). **Cerca** trova chiavi e valori nel documento; il tasto destro copia percorso o valore.
- **Tabella**: i campi di primo livello diventano colonne; sottodocumenti ed elenchi stanno in una cella come JSON. Valgono selezione, copia, pannello valore e grafico.

Dalla palette dei comandi: «Mostra i documenti come albero» e «Mostra i documenti come tabella».

## Modificare un documento
Si modificano i documenti di un `find` senza `projection`.
1. Scegli il documento nella vista **Documenti**.
2. **Modifica** (anche doppio clic o [[Invio]] sulla riga) apre il documento come JSON.
3. Cambia il testo e premi **Salva**. Il documento viene sostituito cercandolo per `_id`, che non cambia.

- **Duplica** apre una copia senza `_id`; **Nuovo documento** ne apre uno vuoto. **Inserisci** lo aggiunge alla collezione.
- **Elimina** toglie il documento per `_id`.
- Se un altro ha eliminato il documento nel frattempo, NiceDB lo dice invece di crearne uno nuovo.

## Produzione e registro
Ogni comando passa dallo stesso controllo delle istruzioni SQL ([Sicurezza in produzione](production)):
- `find`, `aggregate`, `count`, `distinct`, `explain` e gli altri comandi di lettura passano anche a scrittura bloccata;
- `insert`, `update`, `delete`, `drop`, `createIndexes` e ogni comando sconosciuto chiedono lo sblocco; un `aggregate` con `$out` o `$merge` è una scrittura;
- `delete` o `update` su tutti i documenti, `drop`, `dropDatabase`, `dropIndexes`, `collMod`, `renameCollection` chiedono conferma come rischiosi.

Le modifiche dalla vista **Documenti** finiscono nel registro come il comando equivalente.

## Limiti
- Vista Dati, Schema e Strumenti non ci sono per MongoDB.
- Il piano grafico non c'è: scrivi `{ "explain": { "find": … }, "verbosity": "executionStats" }` e leggi il documento.
- Niente transazioni dall'editor, prova a secco, parametri `:nome` e ordinamento dalle intestazioni.
- Niente certificato client TLS, utenti e ruoli.
- Un campo raro può mancare dai campi ricavati, che vengono da 100 documenti.
- Attraverso tunnel o proxy il server si vede come `127.0.0.1`: con **Verifica il certificato** il certificato deve valere anche per quel nome, altrimenti usa **Cifrato, senza verifica**.
- Senza il permesso di fermare le operazioni, [[key:cancel]] ferma il cursore ma non l'operazione già partita.
