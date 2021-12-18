package de.sve.backend.mail;

import java.nio.charset.StandardCharsets;
import java.util.Base64;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.auto.value.AutoValue;

import de.sve.backend.model.events.EventType;
import jakarta.mail.MessagingException;

/**
 * Different SVE mail accounts (possible sender).
 *
 * @author mseele
 */
@AutoValue
public abstract class MailAccount {

	private static final Logger LOG = LoggerFactory.getLogger(MailAccount.class);

	public static boolean checkConnectivity() {
		boolean result = true;
		for (MailAccount account : ALL) {
			try {
				Postman.checkConnectivity(account);
			} catch (MessagingException e) {
				result = false;
				LOG.error("Verify connection to " + account.email() + " failed.", e); //$NON-NLS-1$ //$NON-NLS-2$
			}
		}
		return result;
	}
	
	public static MailAccount FITNESS = MailAccount.create("fitness@sv-eutingen.de", "QE5DTUNQXWd5PyorVC9VZ3JqS3BhaUdkRE9KM1F2MkJmUzVjajslciNkaS1qWzxvV0p7RVdsbUUmMlNtbSQjPnQ8KTU0M0hWS3AzYn1CW0hedF1RLlhoMGtTbFchLWdxQVZXKg=="); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount EVENTS = MailAccount.create("events@sv-eutingen.de", "M2UrblhvWVhZP1kwIXc5ekcsV1dGO3R9aUl5P1cyclhKRHh1cyYybzJBVXA3KHJuPi13ZD9JPmk4VWs4aGtIOEtVKkU7WDtbTDp9NEJ0JXRtLTcsSntDKXMkRS05RFZGVi5ueg=="); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount INFO = MailAccount.create("info@sv-eutingen.de", "WkIhOmN1e28qcm0mYl0yUFpwaUxmeTNEQ1tEbT5MWFksQjhtOC9ncyxpbT1eL2w3YT0wJiQ7czUqTDhxW2gwbzhTITdxXFd7SCx1SX1qVGguPGg4P2ozSUhkbTQ6diw8SCh6ag=="); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount KUNSTRASEN = MailAccount.create("kunstrasen@sv-eutingen.de", "eDM2JWMmeSQxUzJa"); //$NON-NLS-1$ //$NON-NLS-2$

	public static MailAccount JUGENDTURNIER = MailAccount.create("jugendturnier@sv-eutingen.de", "U3ZlbHIyMDIxIQ=="); //$NON-NLS-1$ //$NON-NLS-2$

	private static MailAccount[] ALL = new MailAccount[] { FITNESS, EVENTS, INFO, KUNSTRASEN, JUGENDTURNIER };

	public static MailAccount of(String emailAddress) {
		if (emailAddress != null && emailAddress.length() > 0) {
			for (MailAccount account : ALL) {
				if (emailAddress.equals(account.email())) {
					return account;
				}
			}
		}
		return null;
	}

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
