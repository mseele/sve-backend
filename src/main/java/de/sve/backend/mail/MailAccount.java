package de.sve.backend.mail;

import java.nio.charset.StandardCharsets;
import java.util.Base64;

import com.google.auto.value.AutoValue;

import de.sve.backend.model.events.EventType;

/**
 * Different SVE mail accounts (possible sender).
 * 
 * @author mseele
 */
@AutoValue
public abstract class MailAccount {

	public static MailAccount FITNESS = MailAccount.create("fitness@sv-eutingen.de", "QE5DTUNQXWd5PyorVC9VZ3JqS3BhaUdkRE9KM1F2MkJmUzVjajslciNkaS1qWzxvV0p7RVdsbUUmMlNtbSQjPnQ8KTU0M0hWS3AzYn1CW0hedF1RLlhoMGtTbFchLWdxQVZXKg=="); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount EVENTS = MailAccount.create("events@sv-eutingen.de", "M2UrblhvWVhZP1kwIXc5ekcsV1dGO3R9aUl5P1cyclhKRHh1cyYybzJBVXA3KHJuPi13ZD9JPmk4VWs4aGtIOEtVKkU7WDtbTDp9NEJ0JXRtLTcsSntDKXMkRS05RFZGVi5ueg=="); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount INFO = MailAccount.create("info@sv-eutingen.de", "WkIhOmN1e28qcm0mYl0yUFpwaUxmeTNEQ1tEbT5MWFksQjhtOC9ncyxpbT1eL2w3YT0wJiQ7czUqTDhxW2gwbzhTITdxXFd7SCx1SX1qVGguPGg4P2ozSUhkbTQ6diw8SCh6ag=="); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount KUNSTRASEN = MailAccount.create("kunstrasen@sv-eutingen.de", "eDM2JWMmeSQxUzJa"); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount of(EventType type) {
		switch (type) {
			case Events:
				return EVENTS;
			case Fitness:
				return FITNESS;
			default:
				throw new IllegalArgumentException("Event type '" + type + "' is not supported."); //$NON-NLS-1$ //$NON-NLS-2$
		}
	}

	private static MailAccount create(String email, String password) {
		byte[] data = Base64.getDecoder().decode(password);
		return new AutoValue_MailAccount(email, new String(data, StandardCharsets.UTF_8));
	}

	public abstract String email();

	public abstract String password();

	public String userName() {
		return email();
	}

}
