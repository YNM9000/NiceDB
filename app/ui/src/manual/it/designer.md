# Editor visuale della tabella

L'editor visuale cambia la struttura di una tabella, o ne crea una nuova, senza scrivere `ALTER TABLE` a mano. Mentre si modifica, a destra si legge lo script esatto che verrà eseguito.

## Aprire l'editor
- Tasto destro su una tabella, nell'esploratore o su un riquadro dello schema ER › **Modifica tabella…**
- Tasto destro su uno schema o sul gruppo **Tabelle** › **Nuova tabella…**: si apre la stessa scheda, vuota.
- Dalla palette dei comandi ([[key:palette_commands]]): «Modifica la struttura della tabella» con una tabella aperta nella vista **Dati**, oppure «Nuova tabella…».

La scheda si chiama «clienti · struttura» e parte dalla tabella com'è sul server.

## Le quattro parti
| Parte | Cosa si cambia |
|---|---|
| **Colonne** | **Nome**, **Tipo**, **Lungh.**, **NULL**, **Predefinito**, **Auto** (numerazione automatica), **Commento**; la chiave accanto al nome mette o toglie la colonna dalla chiave primaria; le frecce cambiano l'ordine |
| **Indici** | nome, **Colonne, in ordine**, **Unico** |
| **Chiavi esterne** | nome, **Tabella di riferimento**, coppie di colonne (**Altra colonna** ne aggiunge una), **Alla cancellazione**, **All'aggiornamento** |
| **Controlli** | nome e **Condizione**, per esempio `prezzo >= 0` |

- **Tipo** propone i tipi del motore (`varchar`, `numeric`, `nvarchar`, `INT8`…) ma accetta qualunque tipo.
- **Lungh.** è la parte tra parentesi: `80` per `varchar(80)`, `10,2` per `numeric(10,2)`, `max` per `nvarchar(max)`.
- **Predefinito** è un'espressione SQL come la vuole il motore: `0`, `'Brescia'`, `now()`, `CURRENT_TIMESTAMP`.
- Un segno verde a sinistra indica una colonna nuova, uno giallo una colonna cambiata.
- Rinominare una colonna aggiorna chiave primaria, indici e chiavi esterne che la usano.
- **Elimina** toglie una colonna anche da chiavi e indici; la colonna finisce in **Da eliminare**, dove **Ripristina** la rimette.
- **Annulla modifiche** riporta tutto com'è sul server.

## Lo script
A destra, **Script** si riscrive a ogni modifica: `CREATE TABLE` per una tabella nuova, gli `ALTER TABLE` nell'ordine sicuro per una esistente. Sopra lo script le note dicono cosa comporta:

| Quando | Nota |
|---|---|
| si toglie una colonna | «La colonna note viene eliminata con i suoi dati.» |
| SQLite, libSQL o DuckDB non sanno fare il cambio sul posto | «… non modifica questa parte sul posto: la tabella viene ricreata e le righe copiate.» |
| MySQL o MariaDB spostano una colonna | «Spostare note riscrive l'intera tabella: su tabelle grandi richiede tempo.» |
| PostgreSQL, CockroachDB o SQL Server e un cambio d'ordine | «… non cambia l'ordine delle colonne: l'ordine resta quello del server.» |
| una colonna nuova NOT NULL senza predefinito | «… fallisce se la tabella ha già righe.» |
| un cambio di tipo | «I valori di prezzo vengono convertiti: se uno non entra nel nuovo tipo, lo script si ferma.» |
| il motore non ha commenti o azioni a cascata | la nota dice cosa non viene scritto |

Finché manca qualcosa (il nome della tabella, il tipo di una colonna, la condizione di un controllo, due colonne con lo stesso nome) lo script resta vuoto e la nota dice cosa sistemare: «Completa i campi segnalati per vedere lo script.»

**Copia script** lo mette negli appunti, **Apri nell'editor** lo scrive in una nuova scheda di query senza eseguirlo.

## Applicare
1. Controllare lo script e le note.
2. Premere **Applica** (per una tabella nuova, **Crea tabella**). Lo script passa dagli stessi controlli dell'editor: analisi di rischio, sblocco e conferma in produzione, registro e cronologia.
3. Su PostgreSQL, SQL Server, SQLite, libSQL e DuckDB lo script gira **in prova**, dentro una transazione: «Script eseguito in prova. Niente è definitivo finché non confermi.»
4. Controllare il risultato, poi **Conferma** per renderlo definitivo o **Annulla** per lasciare la tabella com'era. In produzione la conferma chiede il nome della connessione.
5. Dopo la conferma lo schema si ricarica e la scheda riparte dalla tabella aggiornata.

![Cambiare una colonna nell'editor della tabella e applicare lo script](anim:designer-apply)

Su MySQL, MariaDB e CockroachDB ogni istruzione diventa definitiva appena eseguita: lo script parte subito e, se un'istruzione fallisce, le precedenti restano. Se un'istruzione fallisce, l'errore compare sopra lo script e la prova viene annullata.

## Come cambia la tabella, per motore
| Motore | Come |
|---|---|
| PostgreSQL | `ALTER COLUMN … TYPE … USING`, `SET/DROP NOT NULL`, `SET/DROP DEFAULT`, identità, `COMMENT ON` |
| CockroachDB | come PostgreSQL; la chiave primaria si cambia con `ALTER PRIMARY KEY` e non si può togliere |
| MySQL, MariaDB | `RENAME COLUMN`, `MODIFY COLUMN` con tutta la definizione, `AFTER` o `FIRST` per l'ordine |
| SQL Server | `sp_rename`, `ALTER COLUMN`, predefiniti come vincoli `DF_tabella_colonna`, commenti con `sp_addextendedproperty` |
| SQLite, libSQL | sul posto aggiunta, rinomina ed eliminazione di colonne e indici; il resto ricrea la tabella e ne copia le righe |
| DuckDB | sul posto tipo, NULL, predefiniti, rinomine e commenti su una tabella senza indici; il resto ricrea la tabella |

## Trigger e ricostruzione
Quando SQLite o libSQL devono ricreare la tabella, lo script rimette i trigger dopo gli indici. Se un trigger usa una colonna che la modifica elimina o rinomina, o il vecchio nome della tabella, lo script resta vuoto, **Applica** è spento e la nota dice quale trigger: «Il trigger righe_ordine_prezzo_au usa prezzo, che con questa modifica sparisce: la tabella non si può ricreare.» Si cambia o si toglie il trigger, poi si riprova.

## Limiti
- Le viste che leggono la tabella non vengono toccate; rinominando la tabella i riferimenti in viste e funzioni non si aggiornano.
- Su PostgreSQL una colonna `serial` compare con il predefinito `nextval(…)` e senza la spunta **Auto**.
- Due colonne che si scambiano il nome nella stessa modifica danno un errore del server: va fatto in due passi.
- Indici parziali, su espressioni, con colonne incluse o discendenti compaiono come indici semplici: modificandoli si perdono quelle parti.
- I controlli di SQLite senza nome prendono un nome di comodo (`tabella_check_1`).
- CockroachDB cambia il tipo di una colonna solo nei casi che considera sicuri.
- Su DuckDB la ricostruzione non riesce se un'altra tabella ha una chiave esterna verso questa.
