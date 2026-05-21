- DONE Migrate auth middleware #refactor #[[Security Review]]
  :LOGBOOK:
  CLOCK: [2025-03-04 Tue 09:00:00]--[2025-03-04 Tue 11:23:11] =>  2:23
  :END:
  collapsed:: true
	- Spec: see [[Auth Middleware Spec]]
	- Code:
		- ```python
		  def authenticate(request):
		      token = request.headers.get("Authorization")
		      return verify(token)  # see [[verify helper]]
		  ```
	- Follow-up: ping #[[Marcelo]] about #monitoring
