# GymCoach Backend

GymCoach je mikroservisni backend za rad sa trenerima, klijentima, treninzima, gotovim programima i analitikom treninga.

Servisi:
- `gym-coach-auth` - registracija, login i JWT tokeni
- `gym-coach-user` - profili, coach-client veze i matching po ciljevima
- `gym-coach-training` - evidencija treninga, grupa vezbi i serija
- `gym-coach-program` - read-only programi po nedeljama i danima
- `gym-coach-analytic-recomm` - analitika i preporuka za sledeci trening
- `gym-coach-api-gateway` - jedina ulazna tacka za frontend

## Pokretanje lokalno

Pokreni svaki servis u posebnom terminalu:

```powershell
cargo run -p auth
cargo run -p ntp-gym-coach-user
cargo run -p training
cargo run -p program
cargo run -p analytic-recommendation
cargo run -p api-gateway
```

Portovi:
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

## Demo nalozi

- Coach: `coach@gymcoach.rs` / `coach123`
- Client: `client@gymcoach.rs` / `client123`

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
