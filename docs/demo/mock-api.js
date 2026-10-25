// Fake Qobuz data for the screenshots of the README.
//
// Run it with Playwright against the interface (`npm run dev` in web/, proxying to a server on
// 127.0.0.1:8091): it answers the account, the catalogue, the queue and the log from the albums
// below, and forwards everything else (the settings, the import) to the real server.
//
// The scene is read from the address of the page: /downloads?scene=break. It is one of queue
// (the default), break, rate_limited, paused or empty. Nothing here talks to Qobuz.

async (page) => {
	const ALBUMS = [
		{"id": "faoueax4xvcdc", "title": "Beethoven: Complete Symphonies", "artist": "Marek Janowski"},
		{"id": "etp8lnv68x0kc", "title": "Beethoven: The Complete 9 Symphonies", "artist": "René Leibowitz"},
		{"id": "bewom9hoz95mb", "title": "Beethoven: The Complete Symphonies", "artist": "Antonello Manacorda"},
		{"id": "tko7fqj9zgvwb", "title": "Beethoven: Complete Symphonies", "artist": "Daniel Barenboim"},
		{"id": "v5egdgnkclvyb", "title": "Beethoven: Complete Symphonies", "artist": "Andris Nelsons"},
		{"id": "l7voyl86z34kb", "title": "Beethoven: The Complete Symphonies", "artist": "Gianandrea Noseda"},
		{"id": "bgsgz3u5d7rgb", "title": "Beethoven: Complete Symphonies (Remastered)", "artist": "Herbert Blomstedt"},
		{"id": "0734796500023", "title": "Beethoven: Complete Symphonies", "artist": "Josef Krips"},
		{"id": "5055453630262", "title": "Beethoven: The Complete Nine Symphonies", "artist": "Otto Klemperer"},
		{"id": "0829410479561", "title": "Bach: Goldberg Variations, BWV 988", "artist": "Johann Sebastian Bach"},
		{"id": "atdoxgz1q5ohc", "title": "J. S. Bach: Goldberg Variations, BWV 988", "artist": "Oliver Schnyder"},
		{"id": "oi4tzeewel0na", "title": "Bach: The Goldberg Variations, BWV 988", "artist": "Glenn Gould"},
		{"id": "kn46rpcn1k8lb", "title": "J.S. Bach: Goldberg Variations", "artist": "Víkingur Ólafsson"},
		{"id": "hrp82j6wpwssc", "title": "J. S. Bach: Goldberg Variations", "artist": "Nevermind"},
		{"id": "mx3lg06gqij3a", "title": "Bach: Goldberg Variations, BWV 988", "artist": "Murray Perahia"},
		{"id": "0002894778165", "title": "Bach, J.S.: Goldberg Variations, BWV 988", "artist": "Catrin Finch"},
		{"id": "tobayjix1rkdc", "title": "Bach: Goldberg Variations, BWV 988", "artist": "Ton Koopman"},
		{"id": "zc8l9r1tbrc2b", "title": "J. S. Bach: Goldberg Variations, BWV 988", "artist": "Danny Driver"},
		{"id": "sti1p3jsl2alc", "title": "J. S. Bach: Goldberg Variations, BWV 988", "artist": "Fazil Say"},
		{"id": "5099996592151", "title": "Bach: Cello Suites, BWV 1007 - 1012", "artist": "Pablo Casals"},
		{"id": "uozljemtwirob", "title": "Bach: Cello Suites", "artist": "Anastasia Kobekina"},
		{"id": "yyfz2347jvqlc", "title": "Bach: The Complete Cello Suites", "artist": "Jiří Bárta"},
		{"id": "0696998979628", "title": "Bach: Cello Suites Nos. 1, 5 & 6", "artist": "Yo-Yo Ma"},
		{"id": "0794881993420", "title": "Bach: Cello Suites", "artist": "Ophélie Gaillard"},
		{"id": "z2q0w9d9kh01b", "title": "Bach: Cello Suites 1, 2, 3", "artist": "Pablo Casals"},
		{"id": "0828767875125", "title": "Bach: Cello Suites Nos. 1-6, BWV 1007-1012", "artist": "Yo-Yo Ma"},
		{"id": "o927e96fu36ob", "title": "Bach: Cello Suites Nos. 1 & 2", "artist": "Sonia Wieder-Atherton"},
		{"id": "0724355560551", "title": "Bach: Cello Suites Nos. 2, 3 & 6", "artist": "Mstislav Rostropovich"},
		{"id": "ifqw0pbnbx50a", "title": "J. S. Bach: Cello Suites", "artist": "Miriam Prandi"},
		{"id": "0002894474362", "title": "Mozart: Piano Concertos Nos. 6, 17 & 21", "artist": "Géza Anda"},
		{"id": "5099998461653", "title": "Mozart: The Complete Piano Concertos", "artist": "Daniel Barenboim"},
		{"id": "w3jdrlhvsqw9a", "title": "Mozart: Piano Concertos Nos. 5, 6, 11, 16, 17, 18 & 19", "artist": "Christian Zacharias"},
		{"id": "0886443281714", "title": "Wolfgang Amadeus Mozart: The Piano Concertos", "artist": "Murray Perahia"},
		{"id": "lu049di5tjj6o", "title": "Mozart: Piano Concertos, Vol. 12", "artist": "Jean-Efflam Bavouzet"},
		{"id": "0884977746174", "title": "Mozart: The Complete Piano Concertos", "artist": "Murray Perahia"},
		{"id": "5099963179651", "title": "Mozart: Concertos pour piano n° 20, 21, 23 & 27", "artist": "Daniel Barenboim"},
		{"id": "ebo527otg2lnc", "title": "Mozart: Piano Concertos Nos. 11, 12 & 13", "artist": "Daniel Barenboim"},
		{"id": "udgaqw9wtosca", "title": "Mozart: Complete Piano Concertos", "artist": "Ronald Brautigam"},
		{"id": "bt9lv20q2e0nc", "title": "Mozart: Piano Concertos 20 & 23", "artist": "Olga Pashchenko"},
		{"id": "0635212036327", "title": "Mahler: Symphonies Nos. 1-9", "artist": "Lorin Maazel"},
		{"id": "kj7dm4ir1vuja", "title": "Mahler: Symphony No. 1", "artist": "Valery Gergiev"},
		{"id": "0002894795447", "title": "Mahler: Complete Symphonies", "artist": "Rafael Kubelík"},
		{"id": "by491dzaehpja", "title": "Mahler: Symphony No. 5", "artist": "Valery Gergiev"},
		{"id": "0886443494633", "title": "Leonard Bernstein: The Complete Mahler Symphonies", "artist": "Leonard Bernstein"},
		{"id": "o2z98g5lz28jc", "title": "Mahler: Symphony No. 7", "artist": "Paavo Järvi"},
		{"id": "fsppmd6qlom5b", "title": "Mahler: Symphony No. 10", "artist": "Claudio Abbado"},
		{"id": "5099706308423", "title": "Mahler: Symphony No. 5", "artist": "Leonard Bernstein"},
		{"id": "gf487c4g4ddqc", "title": "Mahler: Symphony No. 1", "artist": "Daniel Harding"},
		{"id": "0002894534162", "title": "Mahler: Symphony No. 5", "artist": "Pierre Boulez"},
		{"id": "khvybrn6xngec", "title": "Brahms: The Symphonies", "artist": "Riccardo Chailly"},
		{"id": "j9n1cptttx83b", "title": "Brahms: The Symphonies", "artist": "Christoph Eschenbach"},
		{"id": "0002894785819", "title": "Johannes Brahms: The Symphonies", "artist": "Riccardo Chailly"},
		{"id": "haqgpgvx82mwc", "title": "Johannes Brahms: Complete Symphonies", "artist": "John Eliot Gardiner"},
		{"id": "0724356274259", "title": "Brahms: Symphonies Nos. 1-4", "artist": "Otto Klemperer"},
		{"id": "0724356951556", "title": "Brahms: Symphonies 1-3 & Ouvertures", "artist": "Eugen Jochum"},
		{"id": "ukkiq8sj0z2na", "title": "Brahms: The Complete Symphonies, Overtures, Haydn Variations, Violin Concerto, Deutsches Requiem & Alto Rhapsody", "artist": "Otto Klemperer"},
		{"id": "ucqf26heconzb", "title": "Brahms: Complete Symphonies", "artist": "Adam Fischer"},
		{"id": "0888608468800", "title": "Brahms: The 4 Symphonies", "artist": "Wilhelm Furtwängler"},
		{"id": "k5okvpywdrjrb", "title": "Brahms: Complete Symphonies", "artist": "John Eliot Gardiner"},
		{"id": "0002894798191", "title": "Schubert: Piano Sonatas D 959 & 960", "artist": "Krystian Zimerman"},
		{"id": "0002894565732", "title": "Schubert: Piano Sonatas Nos. 9, 18, 20 & 21", "artist": "Alfred Brendel"},
		{"id": "0794881650323", "title": "Schubert: Piano Sonatas Nos. 14 & 19", "artist": "Paul Lewis"},
		{"id": "y9adpcxvg213a", "title": "Schubert: Piano Sonatas, D. 157, 537 & 566", "artist": "Elisabeth Leonskaja"},
		{"id": "3149020211526", "title": "Schubert: Piano Sonatas D.840, 850 & 894, Impromptus D 899, Klavierstücke D 946", "artist": "Paul Lewis"},
		{"id": "0710357633324", "title": "Schubert: Piano Sonatas, Vol. 3", "artist": "Vladimir Feltsman"},
		{"id": "bykksqeug7vba", "title": "Schubert: Piano Sonatas D. 959 & 960", "artist": "Dong Hyek Lim"},
		{"id": "qz387ujgcuyoa", "title": "Schubert: 12 Great Piano Sonatas", "artist": "Daniel-Ben Pienaar"},
		{"id": "n844ti6gze7pb", "title": "Schubert: Piano Sonatas, Vol. 2", "artist": "Yasuyo Yano"},
		{"id": "0002894402952", "title": "Schubert: Piano Sonatas Nos. 13 & 21", "artist": "Radu Lupu"},
		{"id": "wrvlhrd1001mb", "title": "Alfred Cortot plays Chopin: Nocturnes, Preludes, Waltzes, Etudes, Ballades, Impromptus", "artist": "Alfred Cortot"},
		{"id": "bhlwa29vjfnjc", "title": "Frédéric Chopin: Etudes, Ballades, Nocturnes & Polonaise-Fantaisie", "artist": "Sviatoslav Richter"},
		{"id": "jxk7vy2bi56ta", "title": "Chopin: Études, Nocturnes & Piano Sonata No. 2", "artist": "Guiomar Novaes"},
		{"id": "0002894470962", "title": "Chopin: The Nocturnes", "artist": "Maria João Pires"},
		{"id": "0002894151172", "title": "Chopin: Nocturnes", "artist": "Daniel Barenboim"},
		{"id": "ltkcecfigltnc", "title": "Nocturnes", "artist": "Frédéric Chopin"},
		{"id": "5410939753222", "title": "Chopin: The Complete Nocturnes", "artist": "Stéphane de May"},
		{"id": "0002894775718", "title": "Chopin: Nocturnes", "artist": "Maurizio Pollini"},
		{"id": "hs3oncnsrmthc", "title": "Chopin: Nocturnes (Complete)", "artist": "Stephen Hough"},
		{"id": "0002894782185", "title": "Chopin: Les Nocturnes (Intégrale)", "artist": "Nelson Freire"},
		{"id": "0730099427920", "title": "Debussy / Ravel: Piano Music", "artist": "Klára Körmendi"},
		{"id": "j0fi6sfqn8dlc", "title": "Debussy, Ravel & Skriabin: Orchestral Works", "artist": "Alexander Gauk"},
		{"id": "0190295712709", "title": "Debussy: The Complete Works", "artist": "Various Artists"},
		{"id": "jgasrwxdftwra", "title": "Debussy: The Complete Orchestral Works", "artist": "Various Artists"},
		{"id": "pmr0em2h0gjyc", "title": "Franck, Ravel and Debussy: Piano Works", "artist": "Ivan Moravec"},
		{"id": "iw7uwnsj3k2fb", "title": "Chopin, Debussy & Ravel", "artist": "Kim Bernard"},
		{"id": "0002894467132", "title": "Ravel: Piano Concertos / Debussy: Fantaisie for Piano & Orchestra", "artist": "Zoltán Kocsis"},
		{"id": "0002894343062", "title": "Ravel - Debussy", "artist": "Paul Paray"},
		{"id": "0747313900237", "title": "Debussy: Complete Orchestral Work", "artist": "Jun Märkl"},
		{"id": "piha0et69s7nb", "title": "Shostakovich: The Complete String Quartets", "artist": "Quatuor Danel"},
		{"id": "vwz4hlgpu733b", "title": "Shostakovich: String Quartets, Vol. 1", "artist": "Quartetto Noûs"},
		{"id": "0077775904154", "title": "Shostakovich: String Quartets Nos. 3, 7 & 8", "artist": "Borodin Quartet"},
		{"id": "q6ngg2a6cx1ta", "title": "Shostakovich: String Quartets Nos. 2, 7 & 10", "artist": "Jerusalem Quartet"},
		{"id": "ri07ewkc9tuva", "title": "Shostakovich: String Quartets Nos. 6, 8 & 11", "artist": "Jerusalem Quartet"},
		{"id": "0896931004251", "title": "Shostakovich: String Quartets Nos. 7, 8 & 9", "artist": "Altius Quartet"},
		{"id": "sroenecgh3wgc", "title": "Shostakovich: Complete String Quartets, Vol. 3, Nos. 13-15", "artist": "Cuarteto Casals"},
		{"id": "0002894788209", "title": "Shostakovich: String Quartets Nos. 1, 8 & 14", "artist": "Borodin Quartet"},
		{"id": "tnht4t1znnm4b", "title": "Shostakovich: String Quartets Nos. 14 and 15", "artist": "Eder Quartet"},
		{"id": "bxzbm25db1luc", "title": "Shostakovich: Complete String Quartets, Vol. 1, Nos. 1-5", "artist": "Cuarteto Casals"},
		{"id": "0825646518340", "title": "Vivaldi: The Four Seasons", "artist": "Il Giardino Armonico"},
		{"id": "0002894100012", "title": "Vivaldi: The Four Seasons", "artist": "I Musici"},
		{"id": "0002894756293", "title": "Vivaldi: The Four Seasons", "artist": "Janine Jansen"},
		{"id": "ifetgdk0smcoc", "title": "Vivaldi: The Four Seasons", "artist": "Renaud Capuçon"},
		{"id": "0886443374911", "title": "Vivaldi: The Four Seasons and Three Concertos for Violin and Orchestra", "artist": "Giuliano Carmignola"},
		{"id": "eoqrn3oez7exb", "title": "Vivaldi: The Four Seasons", "artist": "Jordi Savall"},
		{"id": "rnkpzum5nv0tc", "title": "Vivaldi: Le quattro stagioni (The Four Seasons)", "artist": "Théotime Langlois de Swarte"},
		{"id": "mj4gr985z5tia", "title": "Vivaldi: The Four Seasons", "artist": "Luka Faulisi"},
		{"id": "o4hxf25t6nbdc", "title": "Handel: Messiah, HWV 56", "artist": "John Nelson"},
		{"id": "0002894143962", "title": "Handel: Messiah", "artist": "Georg Solti"},
		{"id": "0825646240524", "title": "Handel: Messiah", "artist": "Emmanuelle Haïm"},
		{"id": "0002894342972", "title": "Handel: Messiah", "artist": "John Eliot Gardiner"},
		{"id": "l0e0e9idsu9oc", "title": "Handel: Messiah, HWV 56", "artist": "Ton Koopman"},
		{"id": "wrz5imk2qqjvz", "title": "Handel: Messiah", "artist": "Peter Whelan"},
		{"id": "vpwmr8k6vhuoa", "title": "Tchaikovsky: Piano Concerto No. 1 - Rachmaninoff: Piano Concerto No. 2", "artist": "Van Cliburn"},
		{"id": "0002894794884", "title": "Rachmaninov & Tchaikovsky: Piano Concertos Nos. 2 & 1", "artist": "Sviatoslav Richter"},
		{"id": "0002894466732", "title": "Rachmaninov: Piano Concerto No. 3 / Tchaikovsky: Piano Concerto No. 1", "artist": "Martha Argerich"},
		{"id": "0002894211812", "title": "Rachmaninov: Piano Concerto No. 2 / Tchaikovsky: Piano Concerto No. 1", "artist": "Jorge Bolet"},
		{"id": "0002894190682", "title": "Tchaikovsky: Piano Concerto No. 1 / Rachmaninov: Préludes", "artist": "Sviatoslav Richter"},
		{"id": "i59fb6m9emafc", "title": "Rachmaninoff: The Piano Concertos & Paganini Rhapsody", "artist": "Yuja Wang"},
		{"id": "mez0fhe2qdstc", "title": "Rachmaninoff: Piano Concertos 1 & 4", "artist": "Leonard Pennario"},
		{"id": "0730099531429", "title": "Haydn: String Quartets Op. 76, Nos. 1-3", "artist": "Kodály Quartet"},
		{"id": "0074362510762", "title": "Haydn: String Quartets, Op. 76, Nos. 1, 2 & 3", "artist": "The Lindsays"},
		{"id": "z8f5nl7samp8a", "title": "Joseph Haydn: String Quartets, Op. 50", "artist": "Quatuor Zaïde"},
		{"id": "wp2q4lz3q9txa", "title": "Haydn: The String Quartets Collection Part 1 \"Kaiser\"", "artist": "Wiener Konzerthausquartett"},
		{"id": "z3ofzo6syr39b", "title": "Haydn: String Quartets Op. 20 No. 4 & Op. 74 No. 3 \"The Horseman\" (Remastered 2025)", "artist": "Guarneri Quartet"},
		{"id": "wq6s74jxfm15a", "title": "Haydn: String Quartets, Op. 64 No. 4, 5 & 6", "artist": "Orlando Quartet"},
		{"id": "p9abx4eu967na", "title": "Haydn: The String Quartets Collection Part 2 \"Die Sieben Worte des Erlösers\"", "artist": "Wiener Konzerthausquartett"},
		{"id": "m3hz7dwrzcxoc", "title": "Haydn: String Quartets, Op. 55", "artist": "Panocha Quartet"},
		{"id": "b72sb67lxpnqa", "title": "Joseph Haydn: The Complete String Quartets played on period instruments", "artist": "Festetics Quartet"},
		{"id": "x3112va8f1gxc", "title": "Purcell: Hail! Bright Cecilia, Z. 328 \"Ode to Saint Cecilia\"", "artist": "John Eliot Gardiner"},
		{"id": "0724356241657", "title": "Monteverdi: Complete Duets", "artist": "Alan Curtis"},
		{"id": "xgczlpd4r33fb", "title": "Purcell: King Arthur, Z. 628", "artist": "John Eliot Gardiner"},
		{"id": "gjrrhqkqub5zb", "title": "Monteverdi: Sacred Vocal Music", "artist": "Emma Kirkby"},
		{"id": "0724348207050", "title": "Monteverdi: Orfeo", "artist": "Charles Medlam"},
		{"id": "d9p0ovsug2bra", "title": "Nielsen: Symphonies Nos. 1-6", "artist": "Colin Davis"},
		{"id": "5055354425059", "title": "Nielsen: Complete Symphonies", "artist": "Ole Schmidt"},
		{"id": "4011222205070", "title": "Nielsen: Symphony No. 1 - Sibelius: Symphony No. 7", "artist": "Okko Kamu"},
		{"id": "0782124205928", "title": "Sibelius: Symphonies Nos. 1-7, En Saga, Finlandia & Night Ride and Sunrise", "artist": "Kurt Sanderling"},
		{"id": "5099997360056", "title": "Sibelius: Symphonies Nos. 1-7, Tapiola, Karelia Suite, Finlandia, The Bard, The Swan of Tuonela", "artist": "Paavo Berglund"},
		{"id": "0724356729957", "title": "The Sibelius Edition: Symphonies 1-7", "artist": "John Barbirolli"},
		{"id": "qzo39sqmjui3a", "title": "Sibelius: Symphonies 1-7; Tapiola; 3 Late Fragments", "artist": "Klaus Mäkelä"},
		{"id": "yqgoxkd1pkkec", "title": "Nielsen: Symphonies Nos. 1 & 3", "artist": "Fabio Luisi"}
	];
	const REAL = 'http://127.0.0.1:8091';

	// ─── Deterministic pseudo-random numbers, so every run draws the same screens ───
	const seeded = (seed) => () => {
		seed = (seed * 1664525 + 1013904223) >>> 0;
		return seed / 4294967296;
	};
	const hash = (text) => [...text].reduce((sum, char) => (sum * 31 + char.charCodeAt(0)) >>> 0, 7);

	const LABELS = ['Deutsche Grammophon', 'Decca', 'Harmonia Mundi', 'Warner Classics', 'Sony Classical', 'Alpha Classics', 'BIS', 'Chandos', 'Naxos', 'PENTATONE', 'Hyperion', 'ECM New Series'];

	/** A cover drawn from the title: a warm gradient and the initial of the composer. */
	const cover = (album) => {
		const seed = hash(album.id);
		const hue = 25 + (seed % 60);
		const flip = seed % 2 ? 0 : 1;
		const letter = album.title.replace(/[^A-Za-zÀ-ÿ]/g, '').charAt(0).toUpperCase();
		const svg =
			`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120">` +
			`<defs><linearGradient id="g" x1="${flip}" y1="0" x2="${1 - flip}" y2="1">` +
			`<stop offset="0" stop-color="hsl(${hue} 38% 30%)"/><stop offset="1" stop-color="hsl(${hue + 18} 46% 16%)"/></linearGradient></defs>` +
			`<rect width="120" height="120" fill="url(#g)"/>` +
			`<circle cx="60" cy="60" r="34" fill="none" stroke="hsl(${hue} 60% 70% / .35)" stroke-width="1.5"/>` +
			`<text x="60" y="76" text-anchor="middle" font-family="Georgia,serif" font-size="46" fill="hsl(${hue} 70% 82%)">${letter}</text></svg>`;
		return 'data:image/svg+xml;utf8,' + encodeURIComponent(svg);
	};

	const catalogue = ALBUMS.map((album) => {
		const seed = hash(album.id);
		const hiRes = seed % 5 !== 0;
		return {
			id: album.id,
			url: `https://play.qobuz.com/album/${album.id}`,
			title: album.title,
			version: null,
			artist: album.artist,
			release_date: `${1958 + (seed % 67)}-0${1 + (seed % 9)}-1${seed % 9}`,
			tracks_count: 6 + (seed % 36),
			bit_depth: hiRes ? 24 : 16,
			sampling_rate: hiRes ? [96, 96, 192, 88.2][seed % 4] : 44.1,
			label: LABELS[seed % LABELS.length],
			thumbnail: cover(album),
			parental_warning: false
		};
	});

	// ─── The queue: thousands of links, as an imported list leaves it ───
	const TOTAL = 2400;
	const FINISHED = 168;
	/** How many items have ended: a queue that waits or rests has hardly begun. */
	const finishedIn = (scene) => (scene === 'queue' ? FINISHED : 5);
	const now = () => Math.floor(Date.now() / 1000);

	const queue = (scene) => {
		const draw = seeded(11);
		const order = [];
		while (order.length < TOTAL) {
			const round = catalogue.map((_, index) => index).sort(() => draw() - 0.5);
			order.push(...round);
		}
		const empty = scene === 'empty';
		const done = finishedIn(scene);
		const running = scene === 'queue' ? FINISHED : -1;
		const items = [];
		for (let index = 0; index < (empty ? 0 : TOTAL); index++) {
			const album = catalogue[order[index]];
			const status = index < done ? 'completed' : index === running ? 'running' : 'pending';
			const failed = status === 'completed' && index % 41 === 7;
			items.push({
				id: `q${String(index).padStart(5, '0')}`,
				url: album.url,
				kind: 'album',
				ref_id: album.id,
				label: album.title,
				sub_label: album.artist,
				thumbnail: album.thumbnail,
				status: failed ? 'failed' : status,
				error: failed ? 'Qobuz does not offer this album for download in your country.' : null,
				enqueued_at: now() - 7200,
				started_at: status === 'pending' ? null : now() - 5400 + index * 25,
				finished_at: status === 'completed' || failed ? now() - 5400 + index * 25 + 22 : null,
				downloaded: status === 'completed' && !failed ? album.tracks_count : status === 'running' ? 7 : 0,
				skipped: 0,
				failures: []
			});
		}
		return items;
	};

	const TRACKS = [
		'Symphony No. 5 in C Minor, Op. 67: I. Allegro con brio',
		'Symphony No. 5 in C Minor, Op. 67: II. Andante con moto',
		'Symphony No. 5 in C Minor, Op. 67: III. Scherzo. Allegro',
		'Symphony No. 5 in C Minor, Op. 67: IV. Allegro',
		'Symphony No. 6 in F Major, Op. 68 "Pastoral": I. Erwachen heiterer Empfindungen'
	];

	const progress = (scene) => {
		const idle = { running: false, item_id: null, percent: 0, album: null, tracks: [], halt: null, wait: null };
		if (scene === 'break') return { ...idle, wait: { reason: 'break', until: now() + 187 } };
		if (scene === 'rate_limited') return { ...idle, wait: { reason: 'rate_limited', until: now() + 118 } };
		if (scene === 'paused') return { ...idle, halt: 'paused' };
		if (scene === 'empty') return idle;
		const current = queue(scene)[FINISHED];
		const shown = catalogue.find((entry) => entry.id === current.ref_id);
		return {
			...idle,
			running: true,
			item_id: current.id,
			percent: 41,
			album: {
				title: shown.title,
				artist: shown.artist,
				release_date: shown.release_date,
				tracks_count: shown.tracks_count,
				genre: 'Classical',
				label: shown.label,
				quality: shown.bit_depth === 24 ? `FLAC ${shown.bit_depth}-bit/${shown.sampling_rate} kHz` : 'FLAC 16-bit/44.1 kHz',
				cover: shown.thumbnail
			},
			tracks: TRACKS.slice(0, 3).map((title, index) => ({
				title,
				index: 8 + index,
				total: shown.tracks_count,
				percent: [88, 54, 17][index],
				bytes_per_second: [3_900_000, 3_400_000, 4_100_000][index]
			}))
		};
	};

	const log = () => {
		const lines = [];
		const add = (event, fields = {}, level = 'normal') => lines.push({ seq: lines.length + 1, level, event, ...fields });
		for (let album = 0; album < 5; album++) {
			const item = catalogue[(album * 7) % catalogue.length];
			add('item_started', { label: item.title });
			add('checking_link');
			for (let number = 1; number <= 3; number++) {
				add('downloading_track', { number, title: TRACKS[(number + album) % TRACKS.length] });
				add('track_transfer', { quality: 'FLAC 24-bit/96 kHz', megabytes: 118.4 + number * 17.2, megabytes_per_second: 3.9 + number / 10 });
				add('done');
			}
			add('item_summary', { downloaded: 3, skipped: 0, failed: 0 });
			add('complete');
			if (album === 2) add('break_started', { seconds: 187 });
			else add('between_albums', { seconds: 4 + album * 3 }, 'detailed');
		}
		add('rate_limited', { seconds: 118 }, 'errors');
		return lines;
	};

	const results = (url) => {
		const query = (url.searchParams.get('q') ?? '').toLowerCase();
		const words = query.split(/\s+/).filter(Boolean);
		const items = catalogue.filter((album) => words.every((word) => `${album.title} ${album.artist}`.toLowerCase().includes(word)));
		const offset = Number(url.searchParams.get('offset') ?? 0);
		const limit = Number(url.searchParams.get('limit') ?? 30);
		return { total: items.length, offset, items: items.slice(offset, offset + limit) };
	};

	const json = (route, body) => route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });

	await page.route(/\/api\//, async (route) => {
		const url = new URL(route.request().url());
		const path = url.pathname;
		const method = route.request().method();
		const scene = new URL(route.request().headers()['referer'] ?? 'http://localhost/').searchParams.get('scene') ?? 'queue';

		if (path === '/api/session' && method === 'GET') {
			return json(route, {
				signed_in: true,
				account: { id: 1, display_name: 'Démonstration', email: null, plan: 'Studio Premier' }
			});
		}
		if (path === '/api/downloads' && method === 'GET') {
			const items = queue(scene);
			return json(route, { queue: items, progress: progress(scene), remaining: items.filter((i) => i.status === 'pending' || i.status === 'running').length });
		}
		if (path === '/api/downloads/events') {
			const body =
				`retry: 3600000\n\nevent: queue\ndata: ${JSON.stringify({ queue: queue(scene) })}\n\n` +
				`event: progress\ndata: ${JSON.stringify(progress(scene))}\n\n`;
			return route.fulfill({ status: 200, contentType: 'text/event-stream', body });
		}
		if (path === '/api/log' && method === 'GET') return json(route, log());
		if (path === '/api/search/albums') return json(route, results(url));
		if (path.startsWith('/api/search/')) return json(route, { total: 0, offset: 0, items: [] });

		// The settings, the import and the rest are the real server's
		// (it refuses a request that says it comes from another address, as this page does)
		const { origin, referer, ...headers } = route.request().headers();
		const answer = await route.fetch({ url: REAL + path + url.search, headers });
		return route.fulfill({ response: answer });
	});

	return { albums: catalogue.length, queue: TOTAL };
}
