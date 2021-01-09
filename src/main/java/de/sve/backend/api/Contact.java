package de.sve.backend.api;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.ContactManager;
import de.sve.backend.model.contact.Emails;
import de.sve.backend.model.contact.Message;
import jakarta.ws.rs.POST;
import jakarta.ws.rs.Path;
import jakarta.ws.rs.Produces;
import jakarta.ws.rs.core.MediaType;

@Path("/contact")
@SuppressWarnings("static-method")
public class Contact {

	private static final Logger LOG = LoggerFactory.getLogger(Contact.class);

	@Path("/message")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void message(Message message) throws BackendException {
		try {
			ContactManager.message(message);
		} catch (Throwable t) {
			String msg = "Error while sending message"; //$NON-NLS-1$
			LOG.error(msg + ": " + message, t); //$NON-NLS-1$
			throw new BackendException(msg, t);
		}
	}

	@Path("/emails")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void email(Emails emails) throws BackendException {
		try {
			ContactManager.emails(emails);
		} catch (Throwable t) {
			String msg = "Error while sending email(s)"; //$NON-NLS-1$
			LOG.error(msg + ": " + emails, t); //$NON-NLS-1$
			throw new BackendException(msg, t);
		}
	}

}
