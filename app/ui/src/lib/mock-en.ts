import type { Api, RunEvent, RunRequest } from './api';

const table = (text: string): [string, string][] =>
  text.split('\n').flatMap((line) => {
    const at = line.indexOf(' = ');
    return at > 0 ? [[line.slice(0, at).trim(), line.slice(at + 3).trim()] as [string, string]] : [];
  });

const KINDS = table(`
Officine Meccaniche = Machine Works
Impianti Elettrici = Electrical Systems
Edilnord = Northbuild
Termoidraulica = Plumbing & Heating
Officine = Works
Serramenti = Windows & Doors
Carpenteria = Steelworks
Falegnameria = Joinery
Autotrasporti = Haulage
Tornerie = Turned Parts
Impianti = Installations
Vetreria = Glassworks
Stampaggi = Pressings
Minuterie = Small Parts
Fonderia = Foundry
Elettromeccanica = Electromechanics
Laminati = Rolled Steel
Ferramenta = Hardware
Arredi = Furniture
Legnami = Timber
Colorificio = Paints
Autofficina = Auto Repairs
Logistica = Logistics
Ceramiche = Ceramics`);

const FAMILIES = table(`
Bassi = Hartley
Valtrompia = Thornton
Gatti = Garner
Pedrali = Prescott
Locatelli = Lockwood
Bonomi = Bowman
Rossi = Reed
Colombo = Collins
Ferrari = Fletcher
Bianchi = Blake
Mangili = Mansell
Pezzotta = Pemberton
Facchinetti = Fairbanks
Ghisleni = Gilmore
Zanardelli = Sanderson
Rota = Rowe
Belotti = Bellamy
Cortinovis = Corbett
Pedretti = Pendleton
Lazzaroni = Lawson
Poli = Pollard
Togni = Tolliver
Ferretti = Ferris
Galli = Gale
Lombardi = Langley
Moretti = Morton
Rinaldi = Rennie
Orione = Orion
Levante = Eastgate
Marini = Whitmore
Vela = Sail`);

const FORMS = table(`
S.r.l. = Ltd
S.p.A. = Inc.
S.n.c. = Partners
S.a.s. = LLP`);

const CODES = table(`
FT = INV
ART- = SKU-
amministrazione = backoffice`);

const STREETS = table(`
Via dei Mille = Mill Lane
Via Roma = Main Street
Via Colombo = Oak Avenue
Via Garibaldi = Park Road
Via Mazzini = Church Street`);

const PHRASES = table(`
Palazzolo sull'Oglio = Bridgeport
Palazzolo sull’Oglio = Bridgeport
Gardone Val Trompia = Stonebridge
Ponte San Pietro = Kingsbridge
Trentino-Alto Adige = Rockies
Emilia-Romagna = Mid-Atlantic
Studio Marini = Whitmore Studio
Ferramenta Bassi = Hartley Hardware
Cliente Bassi = Hartley account
Rossi Serramenti — vecchio = Reed Windows — old
Bassi — produzione dal modello = Hartley — production from template
Bassi — staging dal modello = Hartley — staging from template
Bassi — modello = Hartley — template
Bassi — produzione = Hartley — production
Bassi — reportistica = Hartley — reporting
Bassi — locale = Hartley — local
— gestionale = — sales
Listino — produzione = Price list — production
Sensori — serie storiche = Sensors — time series
Catalogo — cluster = Catalog — cluster
Vetrina — anteprima = Storefront — preview
Ordini — analisi = Orders — analytics
App clienti = Customer app
Server remoti = Remote servers
Archivio 2024 = Archive 2024
Gestionale documenti = Sales documents
Gestionale Oracle = Sales on Oracle
Gestione Db2 = Operations on Db2
Contabilità HANA = Accounting on HANA
Magazzino Firebird = Warehouse on Firebird
Magazzino Informix = Warehouse on Informix
Cache gestionale = Sales cache
Analisi vendite = Sales analytics
Ordini per cliente = Orders by customer
Catalogo DynamoDB = DynamoDB catalog
Ricerca ordini = Order search
Ordini Web = Web orders
Anagrafica clienti = Customer master data
Undici cifre, senza IT = Eleven digits, no country prefix
Fatture scadute non pagate = Overdue unpaid invoices
Fatture da incassare = Invoices to collect
Report/Incassi = Reports/Collections
Clienti per provincia = Customers by state
Righe senza articolo = Lines without a product
Dimensione delle tabelle = Table sizes
Ultimi ordini = Latest orders
Fatturato per mese = Turnover by month
Somma delle fatture per mese = Invoice totals by month
Fatture dell’anno = This year's invoices
Ragione sociale = Company name
Partita IVA = Tax ID
cliente dal = customer since
orario ritiro = pickup hours
consegna al piano, citofonare prima = deliver to the floor, ring first
ritiro in sede = collect on site
Fattura 2026/7240 = Invoice 2026/7240
caos: errore inventato = chaos: made-up error
la colonna «città» non esiste = column "town" does not exist
Forse intendevi «clienti.citta». = Perhaps you meant to reference the column "customers.city".
errore di sintassi vicino a « = syntax error at or near "
errore di sintassi in « = syntax error at or near "
la colonna « = column "
» non esiste = " does not exist
ns does not exist: gestionale. = ns does not exist: sales.
SQL injection nella ricostruzione dell'argomento di EXTRACT = SQL injection when rebuilding the argument of EXTRACT
ascii() legge oltre la fine del buffer = ascii() reads past the end of the buffer
Consegna concordata con il magazzino di Seriate. = Delivery agreed with the Oakdale warehouse.
Il cliente chiede la bolla in duplice copia e il DDT firmato. = The customer wants two copies of the delivery note, signed.
Attenzione: le staffe zincate arrivano in due colli separati, = Note: the galvanized brackets arrive in two separate parcels,
il secondo collo parte dal fornitore solo dopo il saldo della fattura precedente. = the second one leaves the supplier only once the previous invoice is paid.
Telefonare al responsabile acquisti prima delle 10. = Call the purchasing manager before 10.
Staging ricaricato dalla produzione ogni lunedì. I dati di prova si perdono. = Staging is reloaded from production every Monday. Test data is lost.
Server del gestionale, **solo lettura** salvo manutenzione. = The sales server, **read-only** except for maintenance.
## Da ricordare = ## Remember
Backup notturno alle \`02:00\`, non lanciare query pesanti dopo mezzanotte = Nightly backup at \`02:00\`, no heavy queries after midnight
Le fatture si chiudono il 5 del mese = Invoices close on the 5th of the month
Chiesto a Marco l'accesso al replicante = Asked Mark for access to the replica
Indice su = Index on
Prima di ogni UPDATE controllare il numero di righe con un SELECT. = Before any UPDATE, check the row count with a SELECT.
Referente: Anna Bassi, anna@bassi.example. = Contact: Anna Hartley, anna@hartley.example.
Chiusura contabile ogni **5 del mese**. = Books close on the **5th of every month**.
Aggiornamenti automatici e una pagina Informazioni con le licenze. = Automatic updates and an About page with the licences.
Il diagramma ER ricorda lo zoom per ogni database. = The ER diagram remembers the zoom for each database.
Bulloni M8 = M8 bolts
Dadi autobloccanti = Lock nuts
Rondelle piane = Flat washers
Viti autofilettanti = Self-tapping screws
Tasselli in nylon = Nylon wall plugs
Staffe zincate = Galvanized brackets
Rivetti a strappo = Blind rivets
Barre filettate = Threaded rods
'BG' = 'OH'
"BG" = "OH"
"BS" = "PA"
"MI" = "NY"
"CR" = "IN"
"LC" = "KY"
'BS' = 'PA'
'MI' = 'NY'
'Brescia' = 'Riverton'
Europe/Rome = America/Chicago
bassi.it = hartley.example
esempio.it = example.net
pec.example.it = mail.example.com
/dati/ = /files/
/Scaricati/ = /Downloads/
/Applicazioni/ = /Applications/
/Scrivania/ = /Desktop/
/certificati = /certs
bassi-ottobre.nicedb = hartley-october.nicedb
clienti-nuovi.csv = new-customers.csv
${'${1:data}'} = ${'${1:doc_date}'}
cliente._id_1_data_-1 = customer._id_1_doc_date_-1
data_-1_totale_1 = doc_date_-1_total_1
SUM(dare) - SUM(avere) = SUM(debit) - SUM(credit)
SET dare = 0 = SET debit = 0
indirizzo.via = address.street
nota = note
Righe degli ordini, una per articolo = Order lines, one per product
Formatta = Format
"via" = "street"
Sessioni = Sessions
indirizzo.citta = address.city
indirizzo.provincia = address.state
indirizzo.cap = address.zip`);

const WORDS = table(`
gestionale_archivio = sales_archive
gestionale = sales
righe_ordine = order_lines
ragione_sociale = company_name
partita_iva = tax_id
codice_fiscale = tax_code
prezzo_listino = list_price
data_pagamento = paid_on
aliquota_iva = vat_rate
v_scadenzario = v_due_invoices
totale_cliente = customer_total
aggiorna_totale = update_total
ricalcola_totale = recalc_total
stato_ordine = order_status
registra_fattura = post_invoice
ordini_storici = orders_history
fatture_storiche = invoices_history
ordini_aperti = open_orders
v_ordini_aperti = v_open_orders
vendite_mensili = monthly_sales
fatturato_mensile = monthly_turnover
ultimi_valori = latest_values
misure_orarie = hourly_readings
misure_giornaliere = daily_readings
lettura_ordini = orders_reader
lettura_gestionale = sales_reader
carico_notturno = nightly_load
report_leggero = light_report
etl_notte = etl_nightly
senza_archiviati = no_archived
solo_lombardia = midwest_only
importo_positivo = amount_positive
prezzo_valido = price_valid
creato_il = created_at
aggiornato_il = updated_at
archiviato_il = archived_on
inviato_il = sent_on
valido_dal = valid_from
valido_al = valid_to
giacenza_totale = total_on_hand
chiudi_ordine = close_order
log_eventi = event_log
conferma-ordine = order-confirmation
ordini_per_cliente = orders_by_customer
ordini_per_stato = orders_by_status
per_categoria = by_category
per_causale = by_reason
per_stato = by_status
per_cliente = by_customer
colonna_con_un_nome_molto_lungo = column_with_a_very_long_name
storico_consolidato_per_esercizio_e_centro_di_costo = history_consolidated_by_fiscal_year_and_cost_center
ordini_data_idx = orders_doc_date_idx
pagamenti_data = payments_doc_date
v_sottoscorta = v_low_stock
ordine_id = order_id
articolo_id = product_id
cliente_id = customer_id
fattura_id = invoice_id
fornitore_id = supplier_id
agente_id = agent_id
sensore_id = sensor_id
ordini_stato = orders_status
stato_1 = status_1
fatture_totale_check = invoices_total_check
tr_ordini_totale = tr_orders_total
clienti = customers
cliente = customer
ordini = orders
ordine = order
righe = lines
riga = line
fatture = invoices
fattura = invoice
articoli = products
articolo = product
fornitori = suppliers
fornitore = supplier
agenti = agents
agente = agent
pagamenti = payments
pagamento = payment
listini = price_lists
listino = price_list
magazzino = warehouse
movimenti = movements
movimento = movement
causale = reason
depositi = depots
contabilita = accounting
contabilità = accounting
contabile = accountant
conto = account
conti = accounts
scadenzario = due_list
scadenza = due_date
scadenze = deadlines
scadute = past_due
scaduti = overdue
pagata = paid
pagato = paid
pagate = paid
imponibile = net_amount
iva = vat
aliquota = rate
totale = total
importo = amount
numero = ref
codice = code
descrizione = description
giacenza = on_hand
giacenze = stock_levels
prezzo = price
quantita = quantity
quantità = quantity
qta = qty
sconto = discount
fido = credit_limit
citta = city
città = town
provincia = state
regione = region
indirizzo = address
indirizzi = addresses
um = uom
cap = zip
piva = vat_no
nome = name
cognome = surname
provvigione = commission
zona = territory
telefono = phone
pec = secure_mail
canale = channel
sollecito = reminder
solleciti = reminders
esito = outcome
fatturato = turnover
fatturazione = billing
ricavi = revenue
margine = margin
incasso = collected
incassi = collections
vendite = sales_ops
acquisti = purchasing
personale = staff
produzione = manufacturing
logistica = logistics
assistenza = support
qualita = qa
qualità = quality
contratti = contracts
contratto = contract
spedizioni = shipments
spedizione = shipment
resi = returns
commesse = work_orders
turni = shifts
cespiti = fixed_assets
mutui = loans
lotti = lots
categoria = category
eventi = events
evento = event
messaggio = message
istante = occurred_at
sensori = sensors
sensore = sensor
misure = readings
temperatura = temperature
umidita = humidity
reparto = department
allarmi = alarms
livello = level
giorno = day
profili = profiles
anno = fiscal_year
mese = month_start
venduto = sold
ordinato = ordered
consegna = delivery
consegnato = delivered
corriere = carrier
vettore = carrier_name
spedito = shipped
bozza = draft
confermato = confirmed
chiuso = closed
annullato = cancelled
aperto = pending
aperti = open
promozioni = promotions
storico = history
storici = history
storiche = history
archivio = archive
archiviato = archived
archiviati = archived
azienda = company
sede = site
ufficio = office
utente = username
scade = expires_at
versione = version
carrello = cart
contatore = counter
classifica = ranking
coda = queue
stampa = print
blocco = lock
offerta = offer
catalogo = catalog
vetrina = storefront
anteprima = preview
sessione = user_session
sessioni = app_sessions
lettura = reader
carico = loader
manutenzione = maintenance
ultimo = last
documenti = documents
documento = document
dati = payload
tracciato = layout
allegato = attachment
tipo = label
creato = created
aggiornato = updated
variante = variant
colori = colors
edilizia = construction
industria = industry
amministrazione = backoffice
mensile = monthly
mensili = monthly
urgente = urgent
verifica = review
anagrafica = masterdata
registrazione = posting
dare = debit
avere = credit
stato = status
note = notes
colonna = column
tabella = table
testo = text_value
valore = value
lunghissima = very_long
enorme = huge
annidato = nested
vuoto = empty
binario = binary
remoti = remote
analisti = analysts
segreteria = front_office
consolidato = consolidated
esercizio = fiscal_year
centro = center
costo = cost
calcola = compute
idraulica = plumbing
elettrico = electrical
viteria = screws
utensili = tools
grigio = grey
bolla = delivery_note
colli = parcels
nord = north
Bergamo = Fairview
Brescia = Riverton
Seriate = Oakdale
Lumezzane = Millbrook
Dalmine = Ashford
Treviglio = Greenfield
Chiari = Clearwater
Rovato = Redhill
Lecco = Lakeside
Curno = Cranford
Ospitaletto = Hillcrest
Sarnico = Westlake
Villongo = Elmwood
Gussago = Glendale
Iseo = Lakewood
Milano = Kingston
Cremona = Bradford
Como = Bayview
Monza = Milford
Lombardia = Great Lakes
Veneto = New England
Piemonte = Plains
Toscana = Southeast
Lazio = Gulf Coast
Campania = Southwest
Puglia = Mountain West
Sicilia = Pacific
Liguria = Northwest
Marche = Appalachia
Bulloneria = Fasteners
Giulia = Julia
Marco = Mark
Chiara = Claire
Luca = Luke
Sara = Sarah
Paolo = Paul
Elena = Ellen
Davide = David
Marta = Martha
Stefano = Stephen
mrossi = mreed
Figli = Sons
bassi = hartley
marini = whitmore
rossi = reed
modello = template
protocollo = registry
gestione = operations
attivi = active
disponibile = available
scontrini = receipts
metodo = method
giorni = days
orfane = orphans
spazio = space
fmese = tmonth
controlli = checks
sbagliata = wrong
ferramenta = hardware
scansione = scan`);

const TOKENS = table(`
data = doc_date
DATA = DOC_DATE
BRT = Northline
GLS = Swiftpost
SDA = Parcelway
Metabase = weekly-report`);

const EXACT = table(`
BG = OH
BS = PA
MI = NY
CR = IN
LC = KY
via = street
tavola = drawing
listino = pricing
listino@primary = pricing@primary`);

const ONE_WAY = new Set(['active', 'ACTIVE', 'order', 'Order', 'ORDER', 'open', 'Open', 'OPEN', 'DAY', 'SCAN', 'table', 'Table', 'TABLE', 'column', 'Column', 'COLUMN', 'value', 'Value', 'VALUE', 'note']);
const SHARED_PARTS = new Set(['order', 'name', 'total', 'version', 'day', 'days', 'status', 'active', 'master', 'column', 'table', 'accounts', 'scan', 'archive', 'preview', 'wrong', 'value', 'cost', 'open', 'last', 'state', 'level', 'empty', 'binary', 'nested', 'document', 'documents', 'space', 'lock']);

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const W = '[\\p{L}\\p{N}_]';
const isWord = (c: string | undefined) => !!c && /[\p{L}\p{N}_]/u.test(c);
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&');
const any = (xs: string[]) => [...xs].sort((x, y) => y.length - x.length).map(escape).join('|');
const bounded = (k: string, edge = W) => `${isWord(k[0]) ? `(?<!${edge})` : ''}${escape(k)}${isWord(k.at(-1)) ? `(?!${edge})` : ''}`;
const TOKEN_EDGE = '[\\p{L}\\p{N}_/]';
const variants = (pairs: [string, string][]): [string, string][] =>
  pairs.flatMap(([a, b]) => [[a, b], [cap(a), cap(b)], [a.toUpperCase(), b.toUpperCase()]] as [string, string][]);
const flip = (pairs: [string, string][], all = false) => pairs.filter(([, b]) => all || !ONE_WAY.has(b)).map(([a, b]) => [b, a] as [string, string]);
const first = (pairs: [string, string][]) => {
  const m = new Map<string, string>();
  for (const [a, b] of pairs) if (!m.has(a)) m.set(a, b);
  return m;
};

type Side = { shared?: Set<string>; codes: Map<string, string>; re: RegExp; words: Map<string, string>; parts: Map<string, string>; compounds: string[][]; exact: Map<string, string>; company: RegExp; street: RegExp; firm: (m: RegExpExecArray) => string; road: (m: RegExpExecArray) => string };

function side(words: [string, string][], parts: [string, string][], exact: [string, string][], codes: [string, string][], company: string, street: string, firm: Side['firm'], road: Side['road']): Side {
  const wordMap = first(words);
  const partMap = first(parts);
  const alt = [...wordMap.keys()].sort((x, y) => y.length - x.length).map((k) => bounded(k, TOKEN_WORDS.has(k) ? TOKEN_EDGE : W)).join('|');
  return {
    codes: new Map(codes),
    re: new RegExp(`(?<code>(?<!${W})(?:${any(codes.map(([a]) => a))})\\d+(?!${W}))|(?<co>${company.replace(/\((?!\?)/g, '(?:')})|(?<st>${street.replace(/\((?!\?)/g, '(?:')})|(?<w>${alt})|(?<id>(?<!${W})${W}*_${W}*)`, 'gu'),
    words: wordMap,
    parts: partMap,
    compounds: [...partMap.keys()].filter((k) => k.includes('_')).map((k) => k.split('_')).sort((x, y) => y.length - x.length),
    exact: first(exact),
    company: new RegExp(`^${company}$`, 'u'),
    street: new RegExp(`^${street}$`, 'u'),
    firm,
    road,
  };
}

const map = (pairs: [string, string][], back = false) => new Map(pairs.map(([a, b]) => (back ? [b, a] : [a, b])));
const [kEn, kIt, fEn, fIt, oEn, oIt, sEn, sIt] = [map(KINDS), map(KINDS, true), map(FAMILIES), map(FAMILIES, true), map(FORMS), map(FORMS, true), map(STREETS), map(STREETS, true)];
const keys = (pairs: [string, string][], back = false) => any(pairs.map(([a, b]) => (back ? b : a)));
const WORD_PAIRS = variants(WORDS);
const NAME_PAIRS = [...FAMILIES, ...KINDS, ...FORMS, ...STREETS];
const OUT = [...PHRASES, ...NAME_PAIRS, ...WORD_PAIRS, ...TOKENS];
const TOKEN_WORDS = new Set(TOKENS.flat());

const EN = side(
  OUT,
  WORD_PAIRS,
  EXACT,
  CODES,
  `(?<!${W})(${keys(KINDS)}) (${keys(FAMILIES)})( & Figli)? (${keys(FORMS)})`,
  `(${keys(STREETS)}),? (\\d+)`,
  (m) => `${fEn.get(m[2]!) ?? m[2]}${m[3] ? ' & Sons' : ''} ${kEn.get(m[1]!) ?? m[1]} ${oEn.get(m[4]!) ?? m[4]}`,
  (m) => `${m[2]} ${sEn.get(m[1]!) ?? m[1]}`,
);
const IT = side(
  flip(OUT),
  flip(WORD_PAIRS, true).filter(([en]) => en.includes('_') || !SHARED_PARTS.has(en.toLowerCase())),
  flip(EXACT),
  flip(CODES),
  `(?<!${W})(${keys(FAMILIES, true)})( & Sons)? (${keys(KINDS, true)}) (${keys(FORMS, true)})`,
  `(\\d+) (${keys(STREETS, true)})`,
  (m) => `${kIt.get(m[3]!) ?? m[3]} ${fIt.get(m[1]!) ?? m[1]}${m[2] ? ' & Figli' : ''} ${oIt.get(m[4]!) ?? m[4]}`,
  (m) => `${sIt.get(m[2]!) ?? m[2]} ${m[1]}`,
);

const IT_SHARED = first(flip(WORD_PAIRS, true).filter(([en]) => SHARED_PARTS.has(en.toLowerCase())));
IT.shared = SHARED_PARTS;

function identifier(s: Side, id: string): string {
  const parts = id.split('_');
  const out: { from: string; to: string; shared: boolean }[] = [];
  for (let i = 0; i < parts.length; ) {
    const hit = s.compounds.find((c) => c.every((p, k) => parts[i + k] === p));
    const from = hit ? hit.join('_') : (parts[i] ?? '');
    const shared = !hit && !!s.shared?.has(from.toLowerCase());
    out.push({ from, to: shared ? (IT_SHARED.get(from) ?? from) : (s.parts.get(from) ?? from), shared });
    i += hit ? hit.length : 1;
  }
  const anchored = out.some((p) => !p.shared && p.to !== p.from) || (out.every((p) => p.shared || /^\d+$/.test(p.from)) && out.some((p) => /^\d+$/.test(p.from)));
  return out.map((p) => (p.shared && !anchored ? p.from : p.to)).join('_');
}

type Edit = { at: number; len: number; by: string };

function convert(s: Side, text: string, edits?: Edit[]): string {
  const exact = s.exact.get(text);
  if (exact !== undefined) {
    edits?.push({ at: 0, len: text.length, by: exact });
    return exact;
  }
  return text.replace(s.re, (m: string, ...rest: unknown[]) => {
    const groups = rest.at(-1) as Record<string, string | undefined>;
    const at = rest.at(-3) as number;
    const co = groups.co ? s.company.exec(m) : null;
    const st = groups.st ? s.street.exec(m) : null;
    const prefix = groups.code ? m.replace(/\d+$/, '') : '';
    const by = groups.code ? `${s.codes.get(prefix) ?? prefix}${m.slice(prefix.length)}` : co ? s.firm(co) : st ? s.road(st) : groups.w ? (s.words.get(m) ?? m) : identifier(s, m);
    if (by !== m) edits?.push({ at, len: m.length, by });
    return by;
  });
}

const RULE = /^-{3,}$/;
const NUMERIC = /^\s*[\d.,]+[KMGTP]?(\s+\(\d+\))?\s*$/;

export function realign(text: string): string {
  if (!text.includes('\n|')) return text;
  const lines = text.split('\n');
  const inTable = (l: string) => (l.startsWith('|') && l.endsWith('|') && l.length > 1) || RULE.test(l);
  for (let i = 0; i < lines.length; ) {
    let j = i;
    while (j < lines.length && inTable(lines[j]!)) j++;
    if (j === i) {
      i++;
      continue;
    }
    const rows = lines.slice(i, j).map((l) => (RULE.test(l) ? null : l.slice(1, -1).split('|')));
    const cells = rows.filter((r) => r !== null);
    if (cells.length && cells.every((r) => r.length === cells[0]!.length)) {
      const widths = cells[0]!.map((_, c) => Math.max(...cells.map((r) => r[c]!.length)));
      const line = (r: string[]) => `|${r.map((cell, c) => (NUMERIC.test(cell) ? cell.padStart(widths[c]!) : cell.padEnd(widths[c]!))).join('|')}|`;
      const width = widths.reduce((a, w) => a + w + 1, 1);
      rows.forEach((r, k) => (lines[i + k] = r ? line(r) : '-'.repeat(width)));
    }
    i = j;
  }
  return lines.join('\n');
}

const cache = { en: new Map<string, string>(), it: new Map<string, string>() };
const LETTER = /\p{L}/u;
function cached(toEn: boolean, text: string): string {
  if (!LETTER.test(text)) return text;
  const c = toEn ? cache.en : cache.it;
  const hit = c.get(text);
  if (hit !== undefined) return hit;
  const out = realign(convert(toEn ? EN : IT, text));
  if (text.length < 200) {
    if (c.size > 50_000) c.clear();
    c.set(text, out);
  }
  return out;
}

export const ITALIAN_WORDS = new Set(
  [...WORDS, ...KINDS, ...FAMILIES]
    .map(([a]) => a.toLowerCase())
    .filter((a) => /^[a-zà-ù]{4,}$/.test(a) && !['note', 'dare', 'online'].includes(a)),
);
export const toEnglish = (text: string) => cached(true, text);
export const toItalian = (text: string) => cached(false, text);

const KEY = /^[\p{L}\p{N}_.-]+$/u;
const FIXED = new Set(['kind', 'type', 'type_name', 'method', 'location', 'status', 'state', 'category', 'severity', 'code', 'driver', 'environment', 'dialect', 'tls', 'tint', 'mark', 'on_delete', 'on_update']);
const API_KEYS = new Set(['note', 'data', 'via']);
const keyOf = (k: string, toEn: boolean) => (!toEn || API_KEYS.has(k) || !KEY.test(k) ? k : (EN.parts.get(k) ?? (k.includes('_') ? identifier(EN, k) : k)));

function deep<T>(value: T, toEn: boolean): T {
  if (typeof value === 'string') return cached(toEn, value) as T;
  if (Array.isArray(value)) return value.map((v: unknown) => deep(v, toEn)) as T;
  if (value instanceof Error) {
    const copy = Object.assign(Object.create(Object.getPrototypeOf(value) as object) as Error, deep({ ...value }, toEn));
    Object.defineProperty(copy, 'message', { value: cached(toEn, value.message), enumerable: false, writable: true, configurable: true });
    Object.defineProperty(copy, 'stack', { value: value.stack, enumerable: false, writable: true, configurable: true });
    return copy as T;
  }
  if (value && typeof value === 'object' && Object.getPrototypeOf(value) === Object.prototype) {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) out[keyOf(k, toEn)] = FIXED.has(k) && typeof v === 'string' ? v : deep(v, toEn);
    return out as T;
  }
  return value;
}
export const english = <T,>(v: T): T => deep(v, true);
export const italian = <T,>(v: T): T => deep(v, false);

function offsets(text: string) {
  const edits: Edit[] = [];
  const italianText = convert(IT, text, edits);
  const spans: { en: number; enLen: number; it: number; itLen: number }[] = [];
  let shift = 0;
  for (const e of edits) {
    spans.push({ en: e.at, enLen: e.len, it: e.at + shift, itLen: e.by.length });
    shift += e.by.length - e.len;
  }
  const along = (i: number, from: 'en' | 'it') => {
    const to = from === 'en' ? 'it' : 'en';
    let d = 0;
    for (const s of spans) {
      const start = s[from];
      const len = from === 'en' ? s.enLen : s.itLen;
      if (i < start) break;
      if (i < start + len) return s[to] + Math.min(i - start, to === 'en' ? s.enLen : s.itLen);
      d = s[to] + (to === 'en' ? s.enLen : s.itLen) - (start + len);
    }
    return i + d;
  };
  return { italianText, toEn: (i: number) => along(i, 'it'), toIt: (i: number) => along(i, 'en') };
}

type Run = Api['run'];
function englishRun(run: Run): Run {
  return (req: RunRequest, emit: (e: RunEvent) => void) => {
    const { italianText, toEn, toIt } = offsets(req.text);
    const scope: RunRequest['scope'] =
      req.scope.kind === 'statement' ? { ...req.scope, cursor: toIt(req.scope.cursor) } : req.scope.kind === 'selection' ? { ...req.scope, from: toIt(req.scope.from), to: toIt(req.scope.to) } : req.scope;
    const starts = new Map<number, [number, number]>();
    const line = (at: number) => {
      const before = req.text.slice(0, at);
      return { line: before.split('\n').length, column: at - before.lastIndexOf('\n') };
    };
    return run({ ...italian(req), text: italianText, scope }, (e) => {
      if (e.type === 'statement') {
        const from = toEn(e.from);
        const to = toEn(e.to);
        starts.set(e.index, [e.from, from]);
        emit({ ...e, from, to, sql: req.text.slice(from, to) });
      } else if (e.type === 'failed') {
        const at = e.at == null ? e.at : toEn(e.at);
        const start = starts.get(e.index);
        const offset = e.error.kind === 'query' && start && e.error.offset != null ? toEn(start[0] + e.error.offset) - start[1] : undefined;
        const error = english(e.error);
        emit({ ...e, error: offset === undefined || error.kind !== 'query' ? error : { ...error, offset }, at, position: at == null ? e.position : line(at) });
      } else emit(english(e));
    });
  };
}

const RAW = new Set(['appearanceGet', 'appearanceSet', 'paramsFound']);

export function englishMock(mock: Api, locale: string): Api {
  let on = locale === 'en';
  const out: Record<string, unknown> = {};
  for (const [name, fn] of Object.entries(mock as unknown as Record<string, unknown>)) {
    if (typeof fn !== 'function') {
      out[name] = fn;
      continue;
    }
    const plain = fn as (...xs: unknown[]) => unknown;
    if (RAW.has(name)) {
      out[name] = (...args: unknown[]) => {
        if (name === 'appearanceSet') on = (args[0] as { locale?: string }).locale === 'en';
        const r = plain(...args);
        return name === 'appearanceGet' && r instanceof Promise ? r.then((a: { locale?: string }) => ((on = a.locale === 'en'), a)) : r;
      };
      continue;
    }
    const translated = name === 'run' ? (englishRun(plain as Run) as (...xs: unknown[]) => unknown) : null;
    out[name] = (...args: unknown[]) => {
      if (!on) return plain(...args);
      if (translated) return translated(...args);
      const inside = args.map((a) => (typeof a === 'function' ? (...xs: unknown[]) => (a as (...ys: unknown[]) => unknown)(...xs.map(english)) : italian(a)));
      const result = plain(...inside);
      return result instanceof Promise ? result.then(english, (e: unknown) => Promise.reject(english(e))) : english(result);
    };
  }
  return out as unknown as Api;
}
