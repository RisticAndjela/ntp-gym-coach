# GymCoach Platform

GymCoach je mikroservisna aplikacija za rad sa trenerima, klijentima, treninzima, programima i analitikom.

Perzistencija:
- `postgres` je zajednicka baza za aplikaciju
- `auth`, `user`, `training` i `program` servisi citaju i upisuju svoje podatke direktno u PostgreSQL
- `api-gateway` ostaje samo ulazna tacka i proxy, bez sopstvene trajne memorije
- `analytics` je i dalje stateless servis koji racuna odgovor iz prosledjenog zahteva

Servisi:
- `gym-coach-auth` - registracija, login i JWT tokeni
- `gym-coach-user` - profili, coach-client veze i matching po ciljevima
- `gym-coach-training` - evidencija treninga, grupa vezbi i serija
- `gym-coach-program` - read-only programi po nedeljama i danima
- `gym-coach-analytic-recomm` - analitika i preporuka za sledeci trening
- `gym-coach-api-gateway` - jedina ulazna tacka za frontend
- `gym-coach-frontend` - Angular korisnicki interfejs povezan na gateway

## Pokretanje lokalno

Pokreni backend servise u posebnim terminalima:

```powershell
cargo run -p auth
cargo run -p ntp-gym-coach-user
cargo run -p training
cargo run -p program
cargo run -p analytic-recommendation
cargo run -p api-gateway
```

Pokreni Angular frontend:

```powershell
cd gym-coach-frontend
npm.cmd start
```

Portovi:
- Frontend `4200`
- Gateway `8080`
- Auth `8081`
- User `8082`
- Training `8083`
- Program `8084`
- Analytics `8085`

## Pokretanje kroz Docker

```powershell
docker compose up --build
```

Docker compose podize i Angular frontend na `http://localhost:4200`.
PostgreSQL je dostupan na `localhost:5432`, a podaci ostaju sacuvani u Docker volume-u `postgres_data`.

## Demo nalozi

- Coach: `coach@gymcoach.rs` / `coach123`
- Client: `client@gymcoach.rs` / `client123`

## Frontend funkcionalnosti

- Login i registracija korisnika kroz gateway
- Dashboard sa pregledom profila, treninga i programa
- Upravljanje profilima, ciljevima i coach-client konekcijama
- Kreiranje i pregled treninga
- Read-only pregled programa po nedeljama i danima
- Analitika i preporuka opterecenja / broja ponavljanja

## Glavni endpointi preko gateway-a

- `POST /api/auth/register`
- `POST /api/auth/login`
- `GET /api/auth/me`
- `GET /api/users/profiles`
- `GET /api/users/coaches`
- `GET /api/users/clients/{client_id}/matches`
- `POST /api/users/connections`
- `GET /api/trainings`
- `GET /api/trainings/client/{client_id}`
- `POST /api/trainings`
- `GET /api/trainings/catalog`
- `GET /api/programs`
- `GET /api/programs/{id}`
- `POST /api/analytics/report`
- `POST /api/analytics/recommendation`

Svi endpointi osim `/api/auth/*` prolaze kroz JWT proveru u API Gateway-u.

## Primer login zahteva

```json
{
  "email": "client@gymcoach.rs",
  "password": "client123"
}
```

## Primer analytics zahteva

```json
{
  "client_id": "22222222-2222-2222-2222-222222222222",
  "exercise_name": "Bench Press",
  "client_goals": ["strength", "fat loss"],
  "progression_preference": "progressive_overload",
  "history": [
    {
      "performed_on": "2026-04-10",
      "sets": [
        { "reps": 8, "load_kg": 60.0 },
        { "reps": 8, "load_kg": 62.5 }
      ]
    },
    {
      "performed_on": "2026-04-24",
      "sets": [
        { "reps": 8, "load_kg": 65.0 },
        { "reps": 6, "load_kg": 67.5 }
      ]
    }
  ]
}
```

`progression_preference` moze biti `progressive_overload` ili `stagnation`.
Preporuka sada vraca kompletniji plan po vezbi: broj serija, ciljanu kilazu i broj ponavljanja, uzimajuci u obzir istoriju svih serija i ciljeve klijenta.
