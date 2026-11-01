# Schema ER e piani di esecuzione

Lo **schema ER** disegna le tabelle di un database e le chiavi esterne che le legano. Il **piano di esecuzione** mostra come il server esegue una query, dove va il tempo e cosa si può migliorare.

## Aprire lo schema ER
- Nel selettore delle viste, **Schema**: il diagramma del database della scheda.
- Tasto destro su una tabella nell'esploratore › **Mostra nello schema ER**: il diagramma si apre centrato su quella tabella.

Le tabelle si dispongono da sole al primo accesso; la disposizione resta salvata per la connessione e il database. In alto si legge il conteggio, per esempio «48 tabelle · 61 relazioni». Le tabelle legate tra loro formano **aree** colorate; quelle senza relazioni stanno in un'area a parte, «senza relazioni».

## Muoversi nel diagramma
| Azione | Effetto |
|---|---|
| trascinare lo sfondo, o [[Spazio]] e trascinare | sposta la vista |
| rotella | zoom |
| [[Shift]] e rotella | scorre in orizzontale |
| trascinare una tabella | la sposta; la posizione si salva |
| clic su una tabella | apre il pannello dei dettagli |
| doppio clic su una tabella | apre i dati nella vista **Dati** |
| [[+]], [[-]], [[0]] | ingrandisce, riduce, torna al 100% |
| [[F]] | adatta il diagramma alla finestra |
| frecce | sposta la vista |
| [[Esc]] | toglie la selezione |

I pulsanti in alto fanno lo stesso: **Ingrandisci**, **Riduci**, **Torna al 100%**, **Adatta**. **Riordina** ricalcola la disposizione da capo. La **Panoramica** in basso mostra tutto il diagramma; trascinandola si sposta la vista.

## Trovare e filtrare
- **Trova tabella o colonna**: scrivere, scegliere con [[↑]] [[↓]], [[Invio]] porta la vista sulla tabella.
- **Filtro** apre le caselle degli schemi e, dentro ogni schema, delle aree: togliendo la spunta le tabelle spariscono dal disegno. **Mostra tutto** le rimette. Con il filtro attivo il pulsante dice quante tabelle restano.

## Il pannello dei dettagli
Clic su una tabella: a destra si leggono righe stimate, **Colonne** (con chiave primaria, chiavi esterne, `unico`, `not null`), **Indici**, **Riferimenti** (le tabelle a cui punta) e **Referenziata da**. Le chiavi esterne senza indice sono segnate «senza indice». **Apri i dati** apre la tabella; **Chiudi** chiude il pannello. Senza tabella selezionata il pannello riassume il database: tabelle, relazioni, schemi e aree.

La **Legenda** spiega i segni delle relazioni: uno, molti, uno e uno solo, zero o uno.

## Menu del tasto destro
Su una tabella: **Apri i dati**, **Nuova query SELECT**, **Inquadra le tabelle collegate**, **Mostra DDL**, **Modifica tabella…**, **Genera SQL**, **Dipendenze…**, **Copia nome**, **Copia CREATE** (vedi [DDL e generatori SQL](ddl) e [Editor visuale della tabella](designer)). Sullo sfondo: **Adatta** e **Riordina**.

## Esportare il diagramma
**Esporta** salva il diagramma come **Disegno SVG** o **Immagine PNG**, con i colori dello stile in uso e le tabelle nascoste dal filtro escluse.

## Leggere un piano
1. Nell'editor, mettere il cursore sulla query.
2. Premere **Spiega** nella barra dell'editor (o il comando «Spiega» nella palette, [[key:palette_commands]]).
3. Si apre la scheda **Piano** accanto ai risultati. Se la query ha parametri, prima compare la barra dei parametri.

Di norma NiceDB chiede il piano **misurato**: la query gira davvero e il piano riporta righe e tempi reali. In testata si legge quale dei due si ha: «EXPLAIN ANALYZE · eseguito alle 10:42» oppure «EXPLAIN · stima delle 10:42, niente eseguito».
- Un'istruzione che scrive viene misurata dentro una transazione annullata subito dopo, e il piano lo dice.
- In produzione un'istruzione che scrive non viene eseguita: si ha solo la stima.
- Su MySQL e MariaDB le scritture hanno solo la stima; DuckDB, con una transazione già aperta, anche.
- SQLite e libSQL non misurano i tempi: il piano mostra l'ordine delle letture.

[[key:cancel]] ferma un piano misurato che dura troppo, come una query.

## La scheda Piano
| Comando | Effetto |
|---|---|
| **Disegno** | il piano come un flusso da sinistra a destra: lo spessore è il numero di righe che passano, il tratto occupato è dove va il tempo |
| **Distinta** | l'elenco dei nodi con righe, stime, tempo o costo e quota |
| **Testo** | il piano come lo restituisce il server |
| **Confronta** | affianca il piano precedente della scheda: «Prima e dopo», con «3 volte più veloce» o «più lento» |
| **Solo stima** | richiede il piano senza eseguire la query |
| **Rispiega** | rifà il piano misurato |

Sopra il disegno, **Cosa succede** riassume il piano in una frase («La query ha impiegato 1,2 s. Il passo più pesante è…»), **Dove va il tempo** (o **Dove va il costo**, per una stima) ripartisce il totale tra i nodi.

## Le note
Sotto il piano, **Note** elenca i problemi trovati, ognuno con la gravità (**Grave**, **Da guardare**, **Minore**), una spiegazione e un **Suggerimento** da copiare con **Copia**:
- lettura completa di una tabella grande con un filtro selettivo;
- chiave esterna senza indice;
- stime lontane dal reale, che chiedono di aggiornare le statistiche;
- ordinamenti o hash che finiscono su disco;
- cicli annidati con molti giri;
- funzioni o conversioni di tipo su una colonna indicizzata, che spengono l'indice;
- `SELECT *` dentro un ordinamento, risultati enormi senza `LIMIT`.

Le correzioni sono scritte nel dialetto del motore. Vanno provate prima su un ambiente che non sia produzione.

## Motori
| Motore | Piano misurato | Solo stima |
|---|---|---|
| PostgreSQL | `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` | `EXPLAIN (FORMAT JSON)` |
| CockroachDB | `EXPLAIN ANALYZE (VERBOSE)` | `EXPLAIN (VERBOSE)` |
| MySQL | `EXPLAIN ANALYZE`, solo per le letture | `EXPLAIN FORMAT=JSON` |
| MariaDB | `ANALYZE FORMAT=JSON`, solo per le letture | `EXPLAIN FORMAT=JSON` |
| SQL Server | `SET STATISTICS XML ON` | `SET SHOWPLAN_XML ON` |
| DuckDB | `EXPLAIN (ANALYZE, FORMAT JSON)` | `EXPLAIN (FORMAT JSON)` |
| SQLite, libSQL | non c'è | `EXPLAIN QUERY PLAN` |
| ClickHouse | non c'è | `EXPLAIN json = 1` (vedi [ClickHouse](clickhouse)) |

MongoDB, Redis e Valkey non hanno **Spiega**. Lo schema ER c'è per gli otto motori relazionali; ClickHouse, MongoDB, Redis e Valkey non ce l'hanno.

## Limiti
- Su SQL Server il piano misurato esegue la query per intero sul server, anche se NiceDB ne scarta le righe: per una query lunga conviene **Solo stima**.
- CockroachDB non dà il piano in JSON: una versione che cambia le parole del piano può lasciare nodi senza tipo, che restano visibili in **Testo**.
- DuckDB stima le righe ma non i costi: nella stima la ripartizione è vuota.
- Le note leggono il catalogo per al massimo 3 secondi: su un server molto carico quelle che hanno bisogno di indici e chiavi esterne possono tacere.
- Con migliaia di tabelle il disegno ER arriva dopo qualche istante; oltre qualche migliaio conviene filtrare per schema.
