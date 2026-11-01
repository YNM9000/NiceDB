# Elasticsearch e OpenSearch

Elasticsearch e OpenSearch tengono documenti JSON in indici. NiceDB parla con loro come la loro console: una richiesta con il suo corpo JSON, oppure SQL.

## Connettersi
1. **Nuova connessione**, poi in **Motore** scegli Elasticsearch (vale anche per OpenSearch).
2. **Host** e **Porta** (9200). Con TLS NiceDB usa HTTPS.
3. **Indici (modelli)**: vuoto mostra tutti gli indici; con `ordini*, clienti` l'albero mostra solo quelli.
4. **Utente** e **Password** per l'accesso con utente; con l'utente vuoto, nella **Password** va la chiave API codificata (il valore `encoded` che il cluster restituisce quando la crei); con entrambi vuoti, un cluster senza sicurezza. Password e chiave stanno nel portachiavi.

## Esplorare
Sotto la connessione ci sono **Indici**, **Alias** e **Data stream**, con il numero di documenti. Passando sopra un indice si leggono salute e spazio; sopra un alias, gli indici che raccoglie. Aprendo un indice si vedono i campi della mappatura, con il percorso a punti (`cliente.nome`) e i sottocampi (`cliente.nome.keyword`).

Un clic su un indice apre una ricerca `match_all` di 100 documenti. **Mostra DDL** dà la `PUT` che ricrea l'indice con impostazioni, mappatura e alias.

## Scrivere
```
GET /ordini/_search
{ "query": { "term": { "stato": "aperto" } }, "size": 50 }

SELECT stato, COUNT(*) FROM ordini GROUP BY stato;
```
- Una richiesta comincia con il metodo e il percorso; il corpo JSON sta sulle righe sotto. Le righe con `#` o `//` sono commenti.
- Una riga che comincia con `SELECT` è SQL e va all'endpoint SQL del cluster.
- L'autocompletamento propone metodi, indici ed endpoint dopo il metodo, e nel corpo le chiavi della Query DSL e i campi dell'indice della richiesta.
- Un errore nel JSON è sottolineato mentre scrivi. **Formatta** riscrive i corpi su più righe.
- `_cat/…` riceve `format=json` da solo e arriva come griglia.

## Risultati
I risultati di una ricerca sono documenti con `_id` e `_index` in testa e poi il `_source` nel suo ordine. Una ricerca con `"size": 0` e aggregazioni arriva come griglia: una riga per ogni bucket più interno, con le metriche accanto. Le altre risposte arrivano come un documento.

Nell'SQL di OpenSearch i nomi con trattini o parole riservate vanno tra apici inversi (`` `ordini-2026` ``), in quello di Elasticsearch tra virgolette doppie; NiceDB sceglie da sé in base al prodotto. I comandi rapidi del pannello **Note** sono solo letture (`topn`, `countby`, `dups`, `nulls`, `page`), e **Copia come** non offre `INSERT`, perché l'SQL di questi motori non scrive.

## Modificare i documenti
I documenti di una `_search` senza filtro sul `_source` si modificano per `_id`, nell'indice da cui vengono (anche cercando su un alias). **Salva** riscrive il documento se c'è ancora, **Inserisci** lo crea, **Elimina** lo toglie; dopo ogni scrittura l'indice si aggiorna prima di rispondere. In produzione tutto passa dal controllo delle scritture e finisce nel registro come richiesta.

## Scritture e produzione
`GET`, `HEAD` e le `POST` di sola lettura (`_search`, `_count`, `_msearch`, `_explain`, `_validate`, `_sql`…) sono letture; il resto scrive. `DELETE` di un indice, `_delete_by_query` o `_update_by_query` su tutti i documenti, `_close` e le impostazioni del cluster chiedono conferma come rischiose. [[key:cancel]] annulla anche sul server la richiesta in corso.

## Spiega: il profilo
**Spiega** ripete la `_search` con `"profile": true` e la disegna come un piano: un ramo per shard, con le query di Lucene, i collector e le aggregazioni, ciascuna con il suo tempo.

## Strumenti › Cluster
Salute, versione, nodi, shard attivi e non assegnati; poi le tabelle dei nodi (memoria, CPU, disco), degli indici e degli shard, con lo stato colorato.
