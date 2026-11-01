# Cassandra e ScyllaDB

Cassandra e ScyllaDB tengono le righe in partizioni, divise sui nodi del cluster in base alla chiave. NiceDB le legge in CQL e le mostra come documenti, una riga per documento.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli Cassandra (vale anche per ScyllaDB: NiceDB riconosce il prodotto da solo).
2. **Nodi**: uno o più host separati da virgola, tutti sulla **Porta** indicata (9042). Con un nodo solo NiceDB passa sempre da quello, anche se il cluster annuncia altri indirizzi: così funziona anche un nodo dietro NAT o in un container.
3. **Keyspace**: quello su cui lavorano le query; vuoto per nessuno.
4. **Utente** vuoto vale un cluster senza autenticazione; la password sta nel portachiavi.
5. **Consistenza**: quante repliche devono rispondere a ogni lettura e scrittura, per esempio `LOCAL_QUORUM` (la predefinita) o `ONE`.
6. **Datacenter locale**: con un cluster su più datacenter, quello da preferire.
7. TLS, certificato client, tunnel SSH e proxy funzionano come per gli altri server.

## Esplorare
Sotto la connessione ci sono i keyspace; in ognuno **Tabelle**, **Viste materializzate** e **Tipi**. Passando sopra una tabella si legge la chiave primaria, per esempio `PRIMARY KEY ((cliente, anno), numero DESC)`: tra le doppie parentesi la chiave di partizione, dopo le colonne di clustering con il loro ordine. Aprendola si vedono le colonne in quest'ordine, la chiave evidenziata, e gli indici secondari.

Un clic su una tabella apre `SELECT * FROM tabella LIMIT 100;` e lo esegue. **Mostra DDL** chiede al server il suo `DESCRIBE`.

## Scrivere CQL
- Le istruzioni finiscono con `;`. Un `BEGIN BATCH … APPLY BATCH` resta un'istruzione sola anche con i `;` dentro.
- L'autocompletamento propone parole chiave, tipi, funzioni, keyspace, tabelle e colonne.
- `CONSISTENCY QUORUM` cambia la consistenza della scheda fino a quando la chiudi; `CONSISTENCY` da solo dice quella in uso.
- Nomi con maiuscole o parole riservate vanno tra virgolette doppie (`"Ordini"`, `"token"`): l'autocompletamento, il clic sulla tabella e i comandi rapidi le mettono da soli, solo dove servono.
- I comandi rapidi del pannello **Note** sono quelli che Cassandra accetta: `tx` scrive un `BEGIN BATCH … APPLY BATCH`, `lastn` un `PER PARTITION LIMIT`, `topn` chiede la partizione prima dell'`ORDER BY`; quelli che CQL non ha (join, paginazione con `OFFSET`) non compaiono.
- Le righe arrivano a pagine di 1000 seguendo il paging state del driver, fino al limite di righe. [[key:cancel]] smette di chiedere pagine.

## Risultati
Ogni riga è un documento: **Documenti** mostra a sinistra la chiave primaria e i primi campi, a destra la riga come albero; **Tabella** la mette in griglia. Liste, insiemi, mappe e tipi definiti dall'utente diventano JSON; decimali e interi grandi arrivano come testo, così restano esatti; date e istanti nel formato di `toJson()`. Nella **Tabella**, **Copia come › INSERT** scrive una `INSERT INTO … JSON '…'` per riga, che Cassandra rilegge con i tipi delle colonne.

## Modificare le righe
Le righe di un `SELECT *` su una tabella si possono modificare, cercate per chiave primaria:
- **Modifica** apre la riga come JSON; **Salva** la riscrive con `INSERT … JSON`. Le colonne della chiave non cambiano, anche se le correggi nel testo.
- **Duplica** apre una copia: cambia la chiave prima di **Inserisci**, altrimenti NiceDB dice che la riga esiste già.
- **Elimina** toglie la riga per chiave.

Se un altro ha tolto la riga nel frattempo, NiceDB lo dice invece di crearla di nuovo. In produzione ogni modifica passa dal controllo delle scritture e finisce nel registro come CQL ([Sicurezza in produzione](production)).

## Scritture e produzione
`SELECT`, `DESCRIBE`, `USE`, `LIST` e `CONSISTENCY` sono letture; tutto il resto scrive. `DROP`, `TRUNCATE` e `ALTER` chiedono conferma come rischiose.

## Spiega: il tracing
**Spiega** esegue la lettura con il tracing acceso e la disegna come un piano: un ramo per ogni nodo che ha lavorato, con i suoi eventi in ordine e il tempo di ciascuno. Il passo più lento è evidenziato. Le scritture non si tracciano, perché il tracing le eseguirebbe.

## Strumenti › Cluster
Nome del cluster, versione, nodi con datacenter, rack e token, l'anello dei token con l'intervallo di ciascun nodo, e i keyspace con la loro replica.
