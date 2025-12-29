        GymCoach - mikroservisna aplikacija
          - maskimalna ocena 10 -

1. Definisanje problema
  Personalni treneri i njihovi klijenti često za praćenje napretka i evidenciju individualnih treninga koriste papirne beleške i notese. Problem stvara stalno listanje u prošlost kako bi videli progres klijenata i planiranje budućih treninga na osnovu podataka upisanih hemijskom olovkom, koji umeju često da budu izmrljani, izgubljeni ili prosto zaboravljeni. Ono što je moj cilj za ovaj projekat je razvoj mikroservisne web aplikacije koja omogućava:
    upravljanje korisnicima (treneri i klijenti), evidenciju treninga i programa,
    analizu performansi
    preporuku budućih trening parametara korišćenjem analitike i jednostavnih ML modela.

2. Arhitektura sistema
  Sistem je realizovan kao mikroservisna arhitektura, gde svaki servis ima sopstvenu bazu podataka i jasno definisane odgovornosti. Komunikacija sa frontend aplikacijom se vrši isključivo putem API Gateway-a. Arhitektura omogućava jednostavno proširenje sistema dodatnim proxy i integracionim slojevima u skladu sa potrebama aplikacije.

3. Mikroservisi
   
  3.1 AuthService
	  Mikroservis koji obavlja uloge registracije i autentifikacije korisnika, vodi računa o JWT tokenima i ima role-based access (RBAC) (COACH,CLIENT).
 
  3.2 UserService
	  Mikroservis zadužen za upravljanje korisničkim profilima, povezivanje trenera i klijenata, čuvanje ciljeva klijenata i ponuda(takođe se odnosi na goals) trenera, na osnovu kojih se putem API-ja vrši odgovarajuće uparivanje klijenata sa trenerima.
  
  3.3 TrainingService
	  Mikroservis koji ima praćenje pojedinacnih treninga. Svaki trening ima svoju kategoriju, koja može biti predefinisana ili prethodno sačuvana, ali može uneti i custom (koja će kasnije biti jedna od sačuvanih). Svaki trening ima grupe vežbi koje se rade tokom treninga. Svaka vežba se sastoji iz tipova(princip isti kao kategorije treninga), datuma i statusa treninga, kao i iz serija. Svaka serija čuva broj ponavljanja i opterećenje, na osnovu kog se pravi grafikon i analitika za predikcije budućih vežbi(više u 3e).
  
  3.4 ProgramService
    Pored treninga imamo i unapred definisane programe koji treneri objavljuju, a bilo koji klijent bez njihove pomoći prati i selektuje ukoliko je završio. Programi su neizmenljivi za klijente i dostupni su isključivo u read-only režimu. Strukturirani su po nedeljama i danima.
  
  3.5 Analytics & Recommendation Service
	  Mikroservis koji se bavi analizom istorijskih podataka treninga, generisanjem statističkih izveštaja i grafikona, kao i preporukom parametara za naredni trening, kao što su preporučena kilaža i broj ponavljanja.
  
  3.6 API Gateway
	  API Gateway predstavlja jedinstvenu ulaznu tačku za frontend, rutiranje zahteva ka mikroservisima, validacija autentifikacije i centralizovana kontrala pristupa.

5. Dodatne i napredne funkcionalnosti
  Od dodatnih funkcionalnosti implementiraću rad sa medijskim fajlovima, jedino dostupni u programskim treninzima, i analitiku u formi grafikona za praćenje progresa individualnih klijenata i vežbi koje izvode.
  Što se tiče naprednih funkcionalnosti koristiću Docker kontejnere, i primenu mašinskog učenja za recommended kilažu ili broj ponavljanja. Kod ove funkcionalnosti ideja je praćenje vežbi iste kategorije klijenata, gde pri treniranju modela uzimam u obzir da li se menja kilaža, broj ponavljanja ili imamo stagnaciju. Takođe, pokušaću da uključim u tabelu za treniranje podataka i koliko je vremena prošlo između dva treninga. Za ML koristim znanje iz ORI-ja iz prethodnog semestra.
Baze podataka
Svaki mikroservis poseduje sopstvenu bazu podataka, čime se obezbeđuje slaba sprega između servisa i nezavisnost u razvoju i skaliranju sistema.
