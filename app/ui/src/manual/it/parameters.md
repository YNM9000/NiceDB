# Parametri nelle query

Una query può avere dei segnaposto al posto dei valori. Quando l'istruzione sotto il cursore, o il testo selezionato, ne contiene, sopra i risultati compare la barra **Parametri** con un campo per ciascuno. Nell'editor i segnaposto sono sottolineati.

```sql
select numero, totale, scadenza
from fatture
where cliente_id = :cliente
  and scadenza >= ${dal}
  and totale > ?
```

## Scrivere i segnaposto
| Forma | Esempio | Nota |
|---|---|---|
| `:nome` | `where id = :id` | |
| `${nome}` | `where data >= ${dal}` | utile dove `:` confonderebbe |
| `?` | `where totale > ?` | ognuno è un valore a sé: nella barra diventa `?1`, `?2`… nell'ordine del testo |

Più le forme proprie dei motori: `$1`, `$2`… su PostgreSQL, CockroachDB e DuckDB; `@nome` su SQL Server (non `@@ROWCOUNT`, non le variabili di `DECLARE`, non gli argomenti di `EXEC p @arg = …`); `@nome` e `$nome` su SQLite e libSQL. Dentro stringhe, commenti e blocchi `$$ … $$` non sono segnaposto.

`:id`, `${id}`, `@id` e `$id` sono lo stesso parametro e compaiono una volta sola. Il nome usa lettere, cifre e `_` e non comincia con una cifra.

Non sono segnaposto:
- il testo dentro stringhe, identificatori fra virgolette, commenti e blocchi `$$ … $$`;
- i cast `::int` e le assegnazioni `:=`;
- un `:` attaccato a una parola (`a:b`, `arr[1:n]`) e le etichette MySQL (`ciclo: loop`);
- su PostgreSQL e DuckDB, il `?` usato come operatore JSON (`dati ? 'chiave'`);
- su SQLite, `?2`, che resta il segnaposto numerato del motore.

## Eseguire con i parametri
1. Scrivi la query con i segnaposto e premi [[key:run_statement]].
2. Se un parametro non ha ancora un valore, la query non parte: la barra evidenzia i campi nuovi e mette il cursore nel primo.
3. Scrivi i valori nei campi.
4. Premi [[Invio]] o **Esegui** nella barra: parte proprio quello che stavi eseguendo, istruzione, selezione o script.

![La barra dei parametri riempie :cliente](anim:params-bar)

[[Esc]] in un campo torna all'editor. Dalla palette: «Vai ai parametri» e «Svuota i valori dei parametri». **Esegui tutto** chiede i valori di tutte le istruzioni dello script, con i `?` numerati lungo tutto il testo.

## Tipi
Ogni campo ha accanto il tipo; un clic lo cambia.

| Tipo | Cosa accetta |
|---|---|
| **Testo** | qualsiasi testo, anche vuoto |
| **Numero** | `42`, `-3.5`, `1e6` |
| **Data** | `2026-01-31`, anche con l'ora |
| **Sì/No** | un interruttore **sì** / **no** |
| **NULL** | niente: il valore è `NULL` |
| **Nome SQL** | un nome di tabella o colonna, anche `schema.tabella` |

Finché non scegli un tipo, la barra lo deduce da quello che scrivi: `1042` diventa **Numero**, `2026-01-01` **Data**. Scelto a mano, resta quello. Un valore che non va bene per il suo tipo colora il campo e ferma l'esecuzione: «Il valore di :id non va bene come numero.»

**Nome SQL** serve dove un valore non può stare, cioè al posto di un nome: `select * from ${tabella}`. Il campo diventa giallo e la barra lo ricorda con un avviso. Accetta solo lettere, cifre, `_`, `$` e punti.

## Valori usati
L'orologio accanto a un campo elenca i valori già usati: prima **In questa query**, poi **In altre query** per lo stesso nome. Un clic riprende il valore. NiceDB ne ricorda fino a 12 per nome; riaprendo una query già usata, i campi ripartono dagli ultimi valori.

I valori usati restano in questo computer e non entrano nel pacchetto `.nicedb`.

## Script salvati con parametri
**Salva script** conserva anche i valori dei parametri presenti nel testo; la finestra li elenca prima di salvare. Nell'elenco degli script i nomi dei parametri compaiono accanto ai tag. Aprendo o eseguendo lo script, la barra parte da quei valori; **Aggiorna dall'editor** salva testo e valori di adesso. Vedi [Note, script e snippet](notes).

## Sicurezza
I valori non vengono mai incollati nel testo così come sono:
- testi e date partono come veri parametri del driver su tutti gli otto motori relazionali; numeri, sì/no e NULL entrano nel testo solo dopo il controllo del tipo;
- su PostgreSQL e CockroachDB, se il risultato ha colonne di un tipo che NiceDB non sa leggere in binario uguale al testo del server (`timestamptz`, `interval`, array, tipi delle estensioni), quell'istruzione parte con i valori come letterali con le virgolette del dialetto, che non possono uscire dalla stringa.

La **prova a secco**, la conferma delle istruzioni rischiose e quella della produzione guardano lo SQL con i valori al loro posto: `delete from clienti where id = :id` non è un DELETE senza WHERE. Cronologia e registro delle scritture conservano lo SQL finale. **Spiega** usa la stessa barra.

## Limiti
- Il tipo **Data** non controlla che la data esista: `2026-02-31` lo rifiuta il server.
- La barra non conosce il tipo della colonna confrontata: il tipo si sceglie o si lascia dedurre.
- I valori scritti nella barra si salvano con lo spazio di lavoro e tornano con la scheda.
